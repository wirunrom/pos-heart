use crate::{
    Error, Pos, Result, catalog,
    schema::{held_carts, products, sales},
};
use diesel::prelude::*;
use pos_domain::{Cart, Money, PaymentMethod, PaymentRequest, Sale, SaleLine, SplitPayment};
use std::collections::HashMap;

/// How the bill is paid: one method, or parts by several methods.
enum Paying<'a> {
    Single(PaymentRequest),
    Split(&'a SplitPayment),
}

impl Pos {
    /// Same cart ID returns its original receipt. All writes share one IMMEDIATE transaction.
    pub fn checkout(&mut self, cart: &Cart, cash: Money, now: i64) -> Result<Sale> {
        self.checkout_for_shift(cart, cash, None, now)
    }
    pub fn checkout_for_shift(
        &mut self,
        cart: &Cart,
        cash: Money,
        shift_id: Option<uuid::Uuid>,
        now: i64,
    ) -> Result<Sale> {
        self.checkout_with_payment(
            cart,
            pos_domain::PaymentRequest {
                method: pos_domain::PaymentMethod::Cash,
                amount: cash,
            },
            shift_id,
            now,
        )
    }
    pub fn checkout_with_payment(
        &mut self,
        cart: &Cart,
        payment: PaymentRequest,
        shift_id: Option<uuid::Uuid>,
        now: i64,
    ) -> Result<Sale> {
        self.pay(cart, Paying::Single(payment), shift_id, now)
    }
    /// Pays one bill with several methods (e.g. part cash, part transfer).
    /// Priced without cash rounding; the parts must equal the exact total.
    /// Each part is its own ledger row, so only the cash part reaches the drawer.
    pub fn checkout_split(
        &mut self,
        cart: &Cart,
        split: &SplitPayment,
        shift_id: Option<uuid::Uuid>,
        now: i64,
    ) -> Result<Sale> {
        self.pay(cart, Paying::Split(split), shift_id, now)
    }
    fn pay(
        &mut self,
        cart: &Cart,
        paying: Paying<'_>,
        shift_id: Option<uuid::Uuid>,
        now: i64,
    ) -> Result<Sale> {
        cart.validate()?;
        let tenders = match &paying {
            Paying::Split(split) => Some(split.normalized()?),
            Paying::Single(_) => None,
        };
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            if let Some(json) = sales::table
                .filter(sales::cart_id.eq(cart.id.to_string()))
                .select(sales::receipt_json)
                .first::<String>(conn)
                .optional()?
            {
                return Ok(serde_json::from_str(&json)?);
            }
            let shift = crate::shifts::active(conn, shift_id, now)?;
            let mut staff = crate::staff::authorize(conn, &session, None)?;
            let mut quantities = HashMap::new();
            for line in &cart.lines {
                let quantity = quantities.entry(line.product_id).or_insert(0_u32);
                *quantity = quantity
                    .checked_add(line.quantity)
                    .ok_or(pos_domain::DomainError::Overflow)?;
            }
            let mut catalog = HashMap::new();
            for (id, quantity) in quantities {
                let p = catalog::find_product(conn, id)?;
                if !p.on_sale {
                    return Err(Error::NotForSale(p.name));
                }
                if let Some(stock) = p.stock {
                    let remaining = stock
                        .checked_sub(quantity)
                        .ok_or_else(|| Error::InsufficientStock(p.name.clone()))?;
                    diesel::update(products::table.find(id.to_string()))
                        .set(products::stock.eq(Some(i64::from(remaining))))
                        .execute(conn)?;
                }
                catalog.insert(id, p);
            }
            let mut cost_total = Money::ZERO;
            let mut lines = Vec::with_capacity(cart.lines.len());
            let mut grosses = Vec::with_capacity(cart.lines.len());
            for line in &cart.lines {
                let p = catalog
                    .get(&line.product_id)
                    .ok_or(Error::ProductUnavailable)?;
                // Options are priced from the catalog inside this transaction.
                let (unit, options) =
                    pos_domain::price_with_options(p.price, &p.options, &line.options)?;
                let amount = unit.checked_mul(line.quantity)?;
                if let Some(cost) = p.cost {
                    cost_total = cost_total.checked_add(cost.checked_mul(line.quantity)?)?;
                }
                grosses.push((amount, line.discount, p.service_charge));
                lines.push(SaleLine {
                    product_id: p.id,
                    sku: p.sku.clone(),
                    name: p.name.clone(),
                    unit_price: unit,
                    unit_cost: p.cost,
                    vat_share: Money::ZERO,
                    quantity: line.quantity,
                    note: line.note.clone(),
                    total: amount,
                    stock_tracked: Some(p.stock.is_some()),
                    options,
                    discount_rule: line.discount,
                    discount: Money::ZERO,
                    bill_discount: Money::ZERO,
                    service: Money::ZERO,
                    vat_added: Money::ZERO,
                    rounding: 0,
                    promotion: None,
                });
            }
            // Promotions are applied from the stored list, at payment time.
            let promos = crate::promotion::load(conn)?;
            let promo_lines = cart
                .lines
                .iter()
                .zip(&grosses)
                .map(|(line, (gross, ..))| {
                    let p = catalog
                        .get(&line.product_id)
                        .ok_or(Error::ProductUnavailable)?;
                    Ok(pos_domain::PromoLine {
                        product_id: p.id,
                        category: &p.category,
                        gross: *gross,
                        quantity: line.quantity,
                        manual: line.discount,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let promoted = pos_domain::apply_promotions(
                &promo_lines,
                cart.discount,
                &promos,
                &pos_domain::LocalTime::bangkok(now),
                &cart.coupons,
            )?;
            let name = |id: &uuid::Uuid| promos.iter().find(|p| p.id == *id).map(|p| p.name.clone());
            for ((gross, line), (discount, promo)) in grosses
                .iter_mut()
                .zip(lines.iter_mut())
                .zip(promoted.line_discounts.iter().zip(&promoted.line_promos))
            {
                gross.1 = *discount;
                line.discount_rule = *discount;
                line.promotion = promo.as_ref().and_then(name);
            }
            // Discounts, service charge, VAT on top and cash rounding are
            // recomputed here from catalog prices and the shop's settings.
            let tax_settings = crate::tax::settings(conn)?;
            let pricing = crate::tax::pricing(conn)?;
            let options = pos_domain::PriceOptions::for_bill(
                &pricing,
                &tax_settings,
                cart.service_charge,
                matches!(paying, Paying::Single(p) if p.method == PaymentMethod::Cash),
            );
            let price = pos_domain::price_sale(&grosses, promoted.bill, &options)?;
            // What each promotion took off this bill.
            let mut used = Vec::new();
            for id in promoted.used() {
                let mut amount = Money::ZERO;
                for (promo, priced) in promoted.line_promos.iter().zip(&price.lines) {
                    if *promo == Some(id) {
                        amount = amount.checked_add(priced.discount)?;
                    }
                }
                if promoted.bill_promo == Some(id) {
                    amount = amount.checked_add(price.bill_discount)?;
                }
                used.push(pos_domain::PromoUse {
                    id,
                    name: name(&id).unwrap_or_default(),
                    amount,
                });
            }
            let promo_off = used
                .iter()
                .try_fold(Money::ZERO, |sum, u| sum.checked_add(u.amount))?;
            // Only the cashier's own discounts count toward their limit.
            let manual_off = Money::from_satang(price.discounts().satang() - promo_off.satang())?;
            for (line, priced) in lines.iter_mut().zip(&price.lines) {
                line.discount = priced.discount;
                line.bill_discount = priced.bill_share;
                line.service = priced.service;
                line.vat_added = priced.vat_added;
                line.rounding = priced.rounding;
            }
            let total = price.total;
            // Cashiers may discount up to the shop's limit; more needs approval.
            // Discounts above the actor's own limit go through the role's
            // Discount permission (allow / approval / deny).
            if manual_off.satang() > 0
                && pos_domain::discount_share_bp(manual_off, price.subtotal)
                    > crate::staff::discount_limit(conn, &session)?
            {
                staff = crate::staff::authorize(
                    conn,
                    &session,
                    Some(pos_domain::Permission::Discount),
                )?;
            }
            // VAT on the receipt is taken from the amount actually paid.
            let tax = tax_settings
                .enabled
                .then(|| pos_domain::SaleTax::for_total(&tax_settings, total));
            // `method` is what the sales row records: the single method, or the
            // first non-cash part of a split (its row keeps no cash, the
            // receipt JSON holds the parts, cash handed over and change).
            let (method, cash, change, row_cash, row_change, tenders) = match (&paying, tenders) {
                (Paying::Single(p), _) if p.method == PaymentMethod::Cash => {
                    let change = total.change_from(p.amount)?;
                    (p.method, p.amount, change, p.amount, change, Vec::new())
                }
                (Paying::Single(p), _) => {
                    if p.amount != total {
                        return Err(Error::PaymentTotalChanged);
                    }
                    (p.method, Money::ZERO, Money::ZERO, Money::ZERO, Money::ZERO, Vec::new())
                }
                (Paying::Split(split), Some(parts)) => {
                    if SplitPayment::total(&parts)? != total {
                        return Err(Error::PaymentTotalChanged);
                    }
                    let cash_part = parts
                        .iter()
                        .find(|t| t.method == PaymentMethod::Cash)
                        .map_or(Money::ZERO, |t| t.amount);
                    let change = cash_part.change_from(split.cash_given)?;
                    let method = parts
                        .iter()
                        .find(|t| t.method != PaymentMethod::Cash)
                        .map(|t| t.method)
                        .ok_or(pos_domain::DomainError::InvalidSplit)?;
                    (method, split.cash_given, change, Money::ZERO, Money::ZERO, parts)
                }
                (Paying::Split(_), None) => return Err(pos_domain::DomainError::InvalidSplit.into()),
            };
            let number = diesel::insert_into(sales::table)
                .values((
                    sales::cart_id.eq(cart.id.to_string()),
                    sales::created_at.eq(now),
                    sales::label.eq(&cart.label),
                    sales::total_satang.eq(total.satang()),
                    sales::cash_satang.eq(row_cash.satang()),
                    sales::change_satang.eq(row_change.satang()),
                    sales::receipt_json.eq("{}"),
                    sales::payment_method.eq(method.key()),
                ))
                .returning(sales::number)
                .get_result::<i64>(conn)?;
            let mut sale = Sale {
                id: cart.id,
                number: u64::try_from(number).map_err(|_| Error::CorruptData)?,
                created_at: now,
                label: cart.label.clone(),
                lines,
                total,
                payment_method: method,
                tenders,
                cash,
                change,
                discount_rule: promoted.bill,
                discount: price.bill_discount,
                discount_reason: if cart.has_discount() {
                    cart.discount_reason.trim().to_owned()
                } else {
                    String::new()
                },
                service_charge: price.service_charge,
                service_rate_bp: if price.service_charge.satang() > 0 {
                    options.service_bp
                } else {
                    0
                },
                vat_added: price.vat_added,
                rounding: price.rounding,
                tax,
                staff: staff.clone(),
                promotions: used,
            };
            sale.allocate_vat()?;
            diesel::update(sales::table.find(number))
                .set(sales::receipt_json.eq(serde_json::to_string(&sale)?))
                .execute(conn)?;
            diesel::delete(held_carts::table.find(cart.id.to_string())).execute(conn)?;
            // One ledger row per method; the first keeps the sale's ID.
            let movements = sale
                .paid_by()
                .into_iter()
                .enumerate()
                .map(|(i, part)| pos_domain::CashMovement {
                    id: if i == 0 { sale.id } else { uuid::Uuid::new_v4() },
                    shift_id: shift.id,
                    created_at: now,
                    kind: pos_domain::CashKind::Sale,
                    amount: part.amount,
                    reason: format!("#{}", sale.number),
                    payment_method: part.method,
                    staff: staff.clone(),
                })
                .collect();
            crate::shifts::record_all(conn, shift.clone(), movements)?;
            crate::promotion::count_uses(conn, &promoted.used())?;
            if manual_off.satang() > 0 {
                crate::audit::record(conn, crate::ActivityKind::Discount, &sale.id.to_string(), now, &sale.staff,
                    &serde_json::json!({"receipt": sale.number, "amount_satang": manual_off.satang(),
                        "reason": sale.discount_reason, "line_discounts": sale.lines.iter().map(|l| serde_json::json!({"name": l.name, "discount_satang": l.discount.satang(), "bill_discount_satang": l.bill_discount.satang()})).collect::<Vec<_>>()}))?;
            }
            Ok(sale)
        })
    }
}
