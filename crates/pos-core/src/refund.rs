use crate::{
    Error, Pos, Result,
    schema::{products, sale_refunds, sale_voids, sales},
};
use diesel::prelude::*;
use pos_domain::{DomainError, Money, RefundRequest, RefundedLine, Sale, SaleRefund};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

fn same_request(previous: &SaleRefund, request: &RefundRequest) -> bool {
    let mut a: Vec<(usize, u32)> = previous
        .lines
        .iter()
        .map(|l| (l.line, l.quantity))
        .collect();
    let mut b: Vec<(usize, u32)> = request.lines.iter().map(|l| (l.line, l.quantity)).collect();
    a.sort_unstable();
    b.sort_unstable();
    previous.sale_id == request.sale_id
        && previous.refund_method == request.refund_method
        && previous.reason == request.reason.trim()
        && a == b
}
impl Pos {
    /// Returns some units of a paid sale through its original payment method.
    /// Refund amount, stock return, audit record and the refunding shift's
    /// ledger commit in one IMMEDIATE transaction. The request ID is looked up
    /// before the open-shift gate so a retry after closing returns the original.
    pub fn refund_items(
        &mut self,
        request: &RefundRequest,
        shift_id: Option<Uuid>,
        now: i64,
    ) -> Result<SaleRefund> {
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            if let Some(json) = sale_refunds::table
                .find(request.id.to_string())
                .select(sale_refunds::refund_json)
                .first::<String>(conn)
                .optional()?
            {
                let previous: SaleRefund = serde_json::from_str(&json)?;
                return if same_request(&previous, request) {
                    Ok(previous)
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            request.validate()?;
            let shift = crate::shifts::active(conn, shift_id, now)?;
            let staff =
                crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Refund))?;
            let json = sales::table
                .filter(sales::cart_id.eq(request.sale_id.to_string()))
                .select(sales::receipt_json)
                .first::<String>(conn)
                .optional()?
                .ok_or(Error::SaleNotFound)?;
            let mut sale: Sale = serde_json::from_str(&json)?;
            if sale.tax.as_ref().is_some_and(|tax| tax.vat > Money::ZERO)
                && sale.lines.iter().all(|line| line.vat_share == Money::ZERO)
            {
                sale.allocate_vat()?;
            }
            if sale.id != request.sale_id {
                return Err(Error::CorruptData);
            }
            if request.refund_method != sale.payment_method {
                return Err(Error::InvalidRefundMethod);
            }
            if now < sale.created_at {
                return Err(Error::InvalidVoidTime);
            }
            let voided = diesel::select(diesel::dsl::exists(
                sale_voids::table.filter(sale_voids::sale_id.eq(sale.id.to_string())),
            ))
            .get_result::<bool>(conn)?;
            if voided {
                return Err(Error::SaleVoided);
            }
            // Units already returned per receipt line.
            let mut returned = vec![0_u32; sale.lines.len()];
            for json in sale_refunds::table
                .filter(sale_refunds::sale_id.eq(sale.id.to_string()))
                .select(sale_refunds::refund_json)
                .load::<String>(conn)?
            {
                let earlier: SaleRefund = serde_json::from_str(&json)?;
                for l in earlier.lines {
                    let slot = returned.get_mut(l.line).ok_or(Error::CorruptData)?;
                    *slot = slot.checked_add(l.quantity).ok_or(DomainError::Overflow)?;
                }
            }
            let legacy: HashSet<usize> = request
                .lines
                .iter()
                .filter(|l| {
                    sale.lines
                        .get(l.line)
                        .is_some_and(|line| line.stock_tracked.is_none())
                })
                .map(|l| l.line)
                .collect();
            if !legacy.is_empty() && request.legacy_restock.is_none() {
                return Err(Error::LegacyStockReviewRequired);
            }
            let chosen: HashSet<usize> = request
                .legacy_restock
                .as_deref()
                .unwrap_or_default()
                .iter()
                .copied()
                .collect();
            if !chosen.is_subset(&legacy) {
                return Err(Error::InvalidStockSelection);
            }
            let mut refund = Money::ZERO;
            let mut lines = Vec::with_capacity(request.lines.len());
            let mut restock = HashMap::<Uuid, u32>::new();
            for l in &request.lines {
                let line = sale.lines.get(l.line).ok_or(Error::RefundExceedsSale)?;
                let left = line.quantity.saturating_sub(returned[l.line]);
                if l.quantity > left {
                    return Err(Error::RefundExceedsSale);
                }
                // What was paid for these units, after line and bill discounts.
                let amount = pos_domain::refund_share(
                    line.net(),
                    line.quantity,
                    returned[l.line],
                    l.quantity,
                )?;
                refund = refund.checked_add(amount)?;
                let back = line
                    .stock_tracked
                    .unwrap_or_else(|| chosen.contains(&l.line));
                let restocked = if back { l.quantity } else { 0 };
                if restocked > 0 {
                    let n = restock.entry(line.product_id).or_default();
                    *n = n.checked_add(restocked).ok_or(DomainError::Overflow)?;
                }
                lines.push(RefundedLine {
                    line: l.line,
                    product_id: line.product_id,
                    name: line.name.clone(),
                    quantity: l.quantity,
                    amount,
                    revenue_ex_vat: Some(line.refund_revenue(returned[l.line], l.quantity)?),
                    restocked,
                });
            }
            for (id, quantity) in restock {
                // Archived products still take their units back.
                let (name, stock) = products::table
                    .find(id.to_string())
                    .select((products::name, products::stock))
                    .first::<(String, Option<i64>)>(conn)
                    .optional()?
                    .ok_or(Error::ProductUnavailable)?;
                let stock = stock.ok_or(Error::StockTrackingChanged(name))?;
                let stock = u32::try_from(stock).map_err(|_| Error::CorruptData)?;
                let after = stock.checked_add(quantity).ok_or(DomainError::Overflow)?;
                diesel::update(products::table.find(id.to_string()))
                    .set(products::stock.eq(Some(i64::from(after))))
                    .execute(conn)?;
            }
            lines.sort_by_key(|l| l.line);
            let record = SaleRefund {
                id: request.id,
                sale_id: sale.id,
                refund_method: sale.payment_method,
                created_at: now,
                reason: request.reason.trim().into(),
                refund,
                lines,
                staff: staff.clone(),
            };
            diesel::insert_into(sale_refunds::table)
                .values((
                    sale_refunds::id.eq(record.id.to_string()),
                    sale_refunds::sale_id.eq(sale.id.to_string()),
                    sale_refunds::created_at.eq(now),
                    sale_refunds::reason.eq(&record.reason),
                    sale_refunds::refund_satang.eq(record.refund.satang()),
                    sale_refunds::refund_method.eq(sale.payment_method.key()),
                    sale_refunds::refund_json.eq(serde_json::to_string(&record)?),
                ))
                .execute(conn)?;
            crate::shifts::record(
                conn,
                shift.clone(),
                pos_domain::CashMovement {
                    id: record.id,
                    shift_id: shift.id,
                    created_at: now,
                    kind: pos_domain::CashKind::Refund,
                    amount: record.refund,
                    reason: format!("#{} · {}", sale.number, record.reason),
                    payment_method: sale.payment_method,
                    staff: staff.clone(),
                },
            )?;
            crate::audit::record(
                conn, crate::ActivityKind::Refund, &sale.id.to_string(), now,
                &record.staff, &serde_json::json!({"receipt": sale.number, "amount_satang": record.refund.satang(), "reason": record.reason}),
            )?;
            Ok(record)
        })
    }
}
