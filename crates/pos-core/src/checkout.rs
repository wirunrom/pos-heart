use crate::{
    Error, Pos, Result, catalog,
    schema::{held_carts, products, sales},
};
use diesel::prelude::*;
use pos_domain::{Cart, Money, Sale, SaleLine};
use std::collections::HashMap;

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
        payment: pos_domain::PaymentRequest,
        shift_id: Option<uuid::Uuid>,
        now: i64,
    ) -> Result<Sale> {
        cart.validate()?;
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
                });
            }
            // Discounts, service charge, VAT on top and cash rounding are
            // recomputed here from catalog prices and the shop's settings.
            let tax_settings = crate::tax::settings(conn)?;
            let pricing = crate::tax::pricing(conn)?;
            let options = pos_domain::PriceOptions::for_bill(
                &pricing,
                &tax_settings,
                cart.service_charge,
                payment.method == pos_domain::PaymentMethod::Cash,
            );
            let price = pos_domain::price_sale(&grosses, cart.discount, &options)?;
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
            if price.discounts().satang() > 0
                && pos_domain::discount_share_bp(price.discounts(), price.subtotal)
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
            let (cash, change) = if payment.method == pos_domain::PaymentMethod::Cash {
                (payment.amount, total.change_from(payment.amount)?)
            } else {
                if payment.amount != total {
                    return Err(Error::PaymentTotalChanged);
                }
                (Money::ZERO, Money::ZERO)
            };
            let number = diesel::insert_into(sales::table)
                .values((
                    sales::cart_id.eq(cart.id.to_string()),
                    sales::created_at.eq(now),
                    sales::label.eq(&cart.label),
                    sales::total_satang.eq(total.satang()),
                    sales::cash_satang.eq(cash.satang()),
                    sales::change_satang.eq(change.satang()),
                    sales::receipt_json.eq("{}"),
                    sales::payment_method.eq(payment.method.key()),
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
                payment_method: payment.method,
                cash,
                change,
                discount_rule: cart.discount,
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
            };
            sale.allocate_vat()?;
            diesel::update(sales::table.find(number))
                .set(sales::receipt_json.eq(serde_json::to_string(&sale)?))
                .execute(conn)?;
            diesel::delete(held_carts::table.find(cart.id.to_string())).execute(conn)?;
            crate::shifts::record(
                conn,
                shift.clone(),
                pos_domain::CashMovement {
                    id: sale.id,
                    shift_id: shift.id,
                    created_at: now,
                    kind: pos_domain::CashKind::Sale,
                    amount: sale.total,
                    reason: format!("#{}", sale.number),
                    payment_method: sale.payment_method,
                    staff,
                },
            )?;
            if price.discounts().satang() > 0 {
                crate::audit::record(conn, crate::ActivityKind::Discount, &sale.id.to_string(), now, &sale.staff,
                    &serde_json::json!({"receipt": sale.number, "amount_satang": price.discounts().satang(),
                        "reason": sale.discount_reason, "line_discounts": sale.lines.iter().map(|l| serde_json::json!({"name": l.name, "discount_satang": l.discount.satang(), "bill_discount_satang": l.bill_discount.satang()})).collect::<Vec<_>>()}))?;
            }
            Ok(sale)
        })
    }
}
