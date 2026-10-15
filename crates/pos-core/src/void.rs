use crate::{
    Error, Pos, Result,
    schema::{products, sale_refunds, sale_voids, sales},
};
use diesel::prelude::*;
use pos_domain::{Sale, SaleVoid, StockReturn, VoidRequest};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

impl Pos {
    /// Original-method refund and stock return commit together. Unique sale_id prevents duplicate refunds.
    pub fn void_sale(&mut self, request: &VoidRequest, now: i64) -> Result<SaleVoid> {
        self.void_sale_for_shift(request, None, now)
    }
    pub fn void_sale_for_shift(
        &mut self,
        request: &VoidRequest,
        shift_id: Option<Uuid>,
        now: i64,
    ) -> Result<SaleVoid> {
        let session = self.session.clone();
        let register = self.register;
        self.conn.immediate_transaction(|conn| {
            if let Some(json) = sale_voids::table
                .find(request.sale_id.to_string())
                .select(sale_voids::void_json)
                .first::<String>(conn)
                .optional()?
            {
                return Ok(serde_json::from_str(&json)?);
            }
            request.validate()?;
            let partial = diesel::select(diesel::dsl::exists(
                sale_refunds::table.filter(sale_refunds::sale_id.eq(request.sale_id.to_string())),
            ))
            .get_result::<bool>(conn)?;
            if partial {
                // A void returns the whole total; after a partial refund that
                // would refund some items twice.
                return Err(Error::PartiallyRefunded);
            }
            let shift = crate::shifts::active(conn, register, shift_id, now)?;
            let staff =
                crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Void))?;
            let json = sales::table
                .filter(sales::cart_id.eq(request.sale_id.to_string()))
                .select(sales::receipt_json)
                .first::<String>(conn)
                .optional()?
                .ok_or(Error::SaleNotFound)?;
            let sale: Sale = serde_json::from_str(&json)?;
            if sale.id != request.sale_id {
                return Err(Error::CorruptData);
            }
            if request.refund_method != sale.payment_method {
                return Err(Error::InvalidRefundMethod);
            }
            if now < sale.created_at {
                return Err(Error::InvalidVoidTime);
            }
            let unknown: HashSet<_> = sale
                .lines
                .iter()
                .filter(|line| line.stock_tracked.is_none())
                .map(|line| line.product_id)
                .collect();
            if !unknown.is_empty() && request.legacy_restock.is_none() {
                return Err(Error::LegacyStockReviewRequired);
            }
            let selected: HashSet<_> = request
                .legacy_restock
                .as_deref()
                .unwrap_or_default()
                .iter()
                .copied()
                .collect();
            if !selected.is_subset(&unknown) {
                return Err(Error::InvalidStockSelection);
            }
            let mut quantities = HashMap::<Uuid, u32>::new();
            for line in &sale.lines {
                if line
                    .stock_tracked
                    .unwrap_or_else(|| selected.contains(&line.product_id))
                {
                    let quantity = quantities.entry(line.product_id).or_default();
                    *quantity = quantity
                        .checked_add(line.quantity)
                        .ok_or(pos_domain::DomainError::Overflow)?;
                }
            }
            let mut restocked = Vec::with_capacity(quantities.len());
            for (id, quantity) in quantities {
                // Archive hides products from selling, but cannot prevent returning their stock.
                let (name, stock) = products::table
                    .find(id.to_string())
                    .select((products::name, products::stock))
                    .first::<(String, Option<i64>)>(conn)
                    .optional()?
                    .ok_or(Error::ProductUnavailable)?;
                let stock = stock.ok_or(Error::StockTrackingChanged(name))?;
                let stock = u32::try_from(stock).map_err(|_| Error::CorruptData)?;
                let returned = stock
                    .checked_add(quantity)
                    .ok_or(pos_domain::DomainError::Overflow)?;
                diesel::update(products::table.find(id.to_string()))
                    .set(products::stock.eq(Some(i64::from(returned))))
                    .execute(conn)?;
                restocked.push(StockReturn {
                    product_id: id,
                    quantity,
                });
            }
            restocked.sort_by_key(|line| line.product_id);
            let cancellation = SaleVoid {
                id: Uuid::new_v4(),
                sale_id: sale.id,
                refund_method: sale.payment_method,
                // A split sale returns each part to its own method.
                tenders: sale.tenders.clone(),
                created_at: now,
                reason: request.reason.trim().into(),
                refund: sale.total,
                restocked,
                legacy_stock_reviewed: !unknown.is_empty(),
                staff: staff.clone(),
            };
            diesel::insert_into(sale_voids::table)
                .values((
                    sale_voids::sale_id.eq(sale.id.to_string()),
                    sale_voids::refund_method.eq(sale.payment_method.key()),
                    sale_voids::id.eq(cancellation.id.to_string()),
                    sale_voids::created_at.eq(now),
                    sale_voids::reason.eq(&cancellation.reason),
                    sale_voids::refund_satang.eq(cancellation.refund.satang()),
                    sale_voids::void_json.eq(serde_json::to_string(&cancellation)?),
                ))
                .execute(conn)?;
            let movements = cancellation
                .refunded_by()
                .into_iter()
                .enumerate()
                .map(|(i, part)| pos_domain::CashMovement {
                    id: if i == 0 { cancellation.id } else { Uuid::new_v4() },
                    shift_id: shift.id,
                    created_at: now,
                    kind: pos_domain::CashKind::Refund,
                    amount: part.amount,
                    reason: format!("#{} · {}", sale.number, cancellation.reason),
                    payment_method: part.method,
                    staff: staff.clone(),
                })
                .collect();
            crate::shifts::record_all(conn, shift.clone(), movements)?;
            crate::ingredient::return_for_void(conn, sale.id, sale.number, &staff, now)?;
            crate::tax::credit_note(
                conn,
                &sale,
                crate::tax::Credit {
                    source_id: cancellation.id,
                    reason: &cancellation.reason,
                    lines: sale
                        .lines
                        .iter()
                        .map(|l| pos_domain::CreditLine {
                            name: l.name.clone(),
                            quantity: l.quantity,
                            amount: l.net(),
                        })
                        .collect(),
                    total: cancellation.refund,
                    vat: sale.tax.as_ref().map_or(pos_domain::Money::ZERO, |t| t.vat),
                    staff: &cancellation.staff,
                },
                now,
            )?;
            crate::audit::record(
                conn, crate::ActivityKind::Void, &sale.id.to_string(), now,
                &cancellation.staff, &serde_json::json!({"receipt": sale.number, "amount_satang": cancellation.refund.satang(), "reason": cancellation.reason}),
            )?;
            Ok(cancellation)
        })
    }
}
