use crate::{Error, Pos, Result, schema::products};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Nullable, Text},
};
use pos_domain::{
    CategorySales, DailySummary, DaySales, DomainError, Money, ProductMovement, Sale, SaleVoid,
    SignedMoney,
};
use std::collections::HashMap;
use time::{Date, macros::offset};
use uuid::Uuid;

/// Longest period a single report covers.
pub const MAX_REPORT_DAYS: i64 = 366;

#[derive(QueryableByName)]
struct ActivityRow {
    #[diesel(sql_type = Text)]
    receipt_json: String,
    #[diesel(sql_type = Nullable<Text>)]
    void_json: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    refund_json: Option<String>,
    #[diesel(sql_type = BigInt)]
    occurred_at: i64,
}
fn add(a: &mut u64, n: u64) -> Result<()> {
    *a = a.checked_add(n).ok_or(DomainError::Overflow)?;
    Ok(())
}
fn signed(m: Money, direction: i64) -> SignedMoney {
    // Money is nonnegative i64, so negating it always fits.
    SignedMoney::from_satang(m.satang() * direction)
}
impl Pos {
    /// Payment activity for one Bangkok day. Refunds belong to their own
    /// processing day, including refunds for older sales.
    pub fn daily_summary(&mut self, date: Date) -> Result<DailySummary> {
        self.summary(date, date)
    }
    /// Activity for Bangkok dates `first..=last` (at most 366 days) in one read
    /// transaction. Sales count on their payment day, refunds on their
    /// cancellation day; net can be negative.
    pub fn summary(&mut self, first: Date, last: Date) -> Result<DailySummary> {
        let after_last = last.next_day().ok_or(Error::InvalidReportDate)?;
        let days = (last - first).whole_days() + 1;
        if !(1..=MAX_REPORT_DAYS).contains(&days) {
            return Err(Error::InvalidReportDate);
        }
        let start = first.midnight().assume_offset(offset!(+7)).unix_timestamp();
        let end = after_last
            .midnight()
            .assume_offset(offset!(+7))
            .unix_timestamp();
        self.conn.transaction(|conn| {
            let mut summary = DailySummary {
                date: first.to_string(),
                end_date: last.to_string(),
                ..DailySummary::default()
            };
            let mut day = first;
            for _ in 0..days {
                summary.days.push(DaySales {
                    date: day.to_string(),
                    ..DaySales::default()
                });
                day = day.next_day().ok_or(Error::InvalidReportDate)?;
            }
            let categories: HashMap<String, String> = products::table
                .select((products::id, products::category))
                .load::<(String, String)>(conn)?
                .into_iter()
                .collect();
            let mut products = HashMap::new();
            let mut by_category: HashMap<String, (i64, SignedMoney)> = HashMap::new();
            let mut by_staff: HashMap<Uuid, pos_domain::StaffSales> = HashMap::new();
            let mut credit = |who: &Option<pos_domain::StaffRef>, sale: Option<Money>, refund: Option<Money>| -> Result<()> {
                let Some(who) = who else { return Ok(()) };
                let e = by_staff.entry(who.id).or_insert_with(|| pos_domain::StaffSales {
                    staff_id: who.id,
                    name: who.name.clone(),
                    sale_count: 0,
                    sales: Money::ZERO,
                    refunds: Money::ZERO,
                });
                if let Some(m) = sale {
                    e.sale_count += 1;
                    e.sales = e.sales.checked_add(m)?;
                }
                if let Some(m) = refund {
                    e.refunds = e.refunds.checked_add(m)?;
                }
                Ok(())
            };
            let query = diesel::sql_query(
                "SELECT receipt_json, NULL AS void_json, NULL AS refund_json,
                        created_at AS occurred_at, 1 AS is_sale
                 FROM sales WHERE created_at>=? AND created_at<?
                 UNION ALL
                 SELECT s.receipt_json, v.void_json, NULL, v.created_at AS occurred_at, 0 AS is_sale
                 FROM sale_voids v JOIN sales s ON s.cart_id=v.sale_id
                 WHERE v.created_at>=? AND v.created_at<?
                 UNION ALL
                 SELECT s.receipt_json, NULL, r.refund_json, r.created_at AS occurred_at, 0 AS is_sale
                 FROM sale_refunds r JOIN sales s ON s.cart_id=r.sale_id
                 WHERE r.created_at>=? AND r.created_at<?
                 ORDER BY occurred_at, is_sale DESC",
            )
            .bind::<BigInt, _>(start)
            .bind::<BigInt, _>(end)
            .bind::<BigInt, _>(start)
            .bind::<BigInt, _>(end)
            .bind::<BigInt, _>(start)
            .bind::<BigInt, _>(end);
            let rows =
                query.load_iter::<ActivityRow, diesel::connection::DefaultLoadingMode>(conn)?;
            for row in rows {
                let row = row?;
                let mut sale: Sale = serde_json::from_str(&row.receipt_json)?;
                // Old receipts predate per-line VAT shares. Allocate in memory
                // for the unknown-cost coverage total, never rewrite the receipt.
                if sale.tax.as_ref().is_some_and(|tax| tax.vat > Money::ZERO)
                    && sale.lines.iter().all(|line| line.vat_share == Money::ZERO)
                { sale.allocate_vat()?; }
                let offset_s = row.occurred_at - start;
                let day_index =
                    usize::try_from(offset_s / 86_400).map_err(|_| Error::CorruptData)?;
                let bucket = summary.days.get_mut(day_index).ok_or(Error::CorruptData)?;
                if let Some(json) = row.refund_json {
                    // Partial refund: only the returned units of the listed lines.
                    let refund: pos_domain::SaleRefund = serde_json::from_str(&json)?;
                    if refund.sale_id != sale.id
                        || !sale.paid_by().iter().any(|t| t.method == refund.refund_method)
                    {
                        return Err(Error::CorruptData);
                    }
                    add(&mut summary.refund_count, 1)?;
                    credit(&refund.staff.by, None, Some(refund.refund))?;
                    if let Some(tax) = &sale.tax {
                        let vat = if refund.lines.iter().all(|line| line.revenue_ex_vat.is_some()) {
                            refund.lines.iter().try_fold(Money::ZERO, |total, line| {
                                let revenue = line.revenue_ex_vat.ok_or(Error::CorruptData)?;
                                let vat = Money::from_satang(line.amount.satang() - revenue.satang())?;
                                Ok::<_, Error>(total.checked_add(vat)?)
                            })?
                        } else {
                            pos_domain::vat_included(refund.refund, tax.rate_bp)
                        };
                        summary.vat_refunds = summary.vat_refunds.checked_add(vat)?;
                    }
                    summary
                        .payments
                        .record(refund.refund_method, refund.refund, true)?;
                    summary.refunds = summary.refunds.checked_add(refund.refund)?;
                    bucket.refunds = bucket.refunds.checked_add(refund.refund)?;
                    for l in refund.lines {
                        let line = sale.lines.get(l.line).ok_or(Error::CorruptData)?;
                        let revenue = match l.revenue_ex_vat {
                            Some(revenue) => revenue,
                            None => Money::from_satang(l.amount.satang() - sale.tax.as_ref().map_or(0, |tax| pos_domain::vat_included(l.amount, tax.rate_bp).satang()))?,
                        };
                        summary.profit.record(revenue, line.unit_cost, l.quantity, true)?;
                        add(&mut summary.restocked_units, u64::from(l.restocked))?;
                        let units = -i64::from(l.quantity);
                        let amount = signed(l.amount, -1);
                        let category = categories
                            .get(&line.product_id.to_string())
                            .cloned()
                            .unwrap_or_default();
                        let c = by_category
                            .entry(category)
                            .or_insert((0, SignedMoney::default()));
                        c.0 = c.0.checked_add(units).ok_or(DomainError::Overflow)?;
                        c.1 = c.1.checked_add(amount)?;
                        let product =
                            products.entry(line.product_id).or_insert_with(|| ProductMovement {
                                product_id: line.product_id,
                                sku: line.sku.clone(),
                                name: line.name.clone(),
                                net_quantity: 0,
                                net_sales: SignedMoney::default(),
                            });
                        product.net_quantity = product
                            .net_quantity
                            .checked_add(units)
                            .ok_or(DomainError::Overflow)?;
                        product.net_sales = product.net_sales.checked_add(amount)?;
                    }
                    continue;
                }
                let direction = if let Some(json) = row.void_json {
                    let cancellation: SaleVoid = serde_json::from_str(&json)?;
                    if cancellation.sale_id != sale.id
                        || cancellation.refund != sale.total
                        || cancellation.refund_method != sale.payment_method
                    {
                        return Err(Error::CorruptData);
                    }
                    add(&mut summary.void_count, 1)?;
                    credit(&cancellation.staff.by, None, Some(cancellation.refund))?;
                    if let Some(tax) = &sale.tax {
                        summary.vat_refunds = summary.vat_refunds.checked_add(tax.vat)?;
                    }
                    summary
                        .payments
                        .record_all(&cancellation.refunded_by(), true)?;
                    summary.refunds = summary.refunds.checked_add(cancellation.refund)?;
                    bucket.refunds = bucket.refunds.checked_add(cancellation.refund)?;
                    for r in &cancellation.restocked {
                        add(&mut summary.restocked_units, u64::from(r.quantity))?;
                    }
                    -1_i64
                } else {
                    add(&mut summary.sale_count, 1)?;
                    credit(&sale.staff.by, Some(sale.total), None)?;
                    if let Some(tax) = &sale.tax {
                        summary.vat_sales = summary.vat_sales.checked_add(tax.vat)?;
                    }
                    summary.payments.record_all(&sale.paid_by(), false)?;
                    summary.gross_sales = summary.gross_sales.checked_add(sale.total)?;
                    summary.discounts = summary.discounts.checked_add(sale.discounts())?;
                    summary.service_charges =
                        summary.service_charges.checked_add(sale.service_charge)?;
                    summary.rounding = summary
                        .rounding
                        .checked_add(SignedMoney::from_satang(sale.rounding))?;
                    bucket.gross = bucket.gross.checked_add(sale.total)?;
                    let hour = usize::try_from((offset_s % 86_400) / 3600)
                        .map_err(|_| Error::CorruptData)?
                        .min(23);
                    summary.hourly[hour] = summary.hourly[hour].checked_add(sale.total)?;
                    summary.cash_received = summary.cash_received.checked_add(sale.cash)?;
                    summary.change_given = summary.change_given.checked_add(sale.change)?;
                    1_i64
                };
                for line in sale.lines {
                    let revenue = Money::from_satang(line.net().satang() - line.vat_share.satang())?;
                    summary.profit.record(revenue, line.unit_cost, line.quantity, direction < 0)?;
                    let units = i64::from(line.quantity) * direction;
                    let amount = signed(line.net(), direction);
                    let category = categories
                        .get(&line.product_id.to_string())
                        .cloned()
                        .unwrap_or_default();
                    let c = by_category
                        .entry(category)
                        .or_insert((0, SignedMoney::default()));
                    c.0 = c.0.checked_add(units).ok_or(DomainError::Overflow)?;
                    c.1 = c.1.checked_add(amount)?;
                    let product =
                        products
                            .entry(line.product_id)
                            .or_insert_with(|| ProductMovement {
                                product_id: line.product_id,
                                sku: line.sku.clone(),
                                name: line.name.clone(),
                                net_quantity: 0,
                                net_sales: SignedMoney::default(),
                            });
                    product.sku = line.sku;
                    product.name = line.name;
                    product.net_quantity = product
                        .net_quantity
                        .checked_add(units)
                        .ok_or(DomainError::Overflow)?;
                    product.net_sales = product.net_sales.checked_add(amount)?;
                }
            }
            // Both operands are nonnegative i64, so their difference always fits i64.
            summary.net =
                SignedMoney::from_satang(summary.gross_sales.satang() - summary.refunds.satang());
            summary.top_products = rank(products);
            let mut staff: Vec<_> = by_staff.into_values().collect();
            staff.sort_by(|a, b| b.sales.cmp(&a.sales).then_with(|| a.name.cmp(&b.name)));
            summary.staff = staff;
            let mut categories: Vec<CategorySales> = by_category
                .into_iter()
                .map(|(category, (net_quantity, net_sales))| CategorySales {
                    category,
                    net_quantity,
                    net_sales,
                })
                .collect();
            categories.sort_by(|a, b| {
                b.net_sales
                    .cmp(&a.net_sales)
                    .then_with(|| a.category.cmp(&b.category))
            });
            summary.categories = categories;
            Ok(summary)
        })
    }
}
/// Top ten by net units, then net amount, then a stable product order.
fn rank(products: HashMap<Uuid, ProductMovement>) -> Vec<ProductMovement> {
    let mut ranked: Vec<_> = products.into_values().collect();
    ranked.sort_by(|a, b| {
        b.net_quantity
            .cmp(&a.net_quantity)
            .then_with(|| b.net_sales.cmp(&a.net_sales))
            .then_with(|| a.product_id.cmp(&b.product_id))
    });
    ranked.truncate(10);
    ranked
}
