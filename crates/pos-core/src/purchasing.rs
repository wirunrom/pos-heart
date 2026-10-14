//! All purchase stock writes, receiving snapshots and activity share one transaction.
use crate::{
    Error, Pos, Result,
    schema::{
        ingredient_movements, ingredients, products, purchase_cancellations, purchase_orders,
        purchase_receipts, suppliers,
    },
};
use diesel::prelude::*;
use pos_domain::*;
use uuid::Uuid;
fn supplier(conn: &mut SqliteConnection, id: Uuid) -> Result<Option<Supplier>> {
    suppliers::table
        .find(id.to_string())
        .select(suppliers::supplier_json)
        .first::<String>(conn)
        .optional()?
        .map(|s| Ok(serde_json::from_str(&s)?))
        .transpose()
}
fn order(conn: &mut SqliteConnection, id: Uuid) -> Result<PurchaseOrder> {
    let s = purchase_orders::table
        .find(id.to_string())
        .select(purchase_orders::order_json)
        .first::<String>(conn)
        .optional()?
        .ok_or(Error::PurchaseOrderNotFound)?;
    Ok(serde_json::from_str(&s)?)
}
fn save_order(conn: &mut SqliteConnection, o: &PurchaseOrder) -> Result<()> {
    diesel::update(purchase_orders::table.find(o.id.to_string()))
        .set((
            purchase_orders::revision.eq(o.revision),
            purchase_orders::order_json.eq(serde_json::to_string(o)?),
        ))
        .execute(conn)?;
    Ok(())
}
fn ingredient(conn: &mut SqliteConnection, id: Uuid) -> Result<Ingredient> {
    let (json, stock) = ingredients::table
        .find(id.to_string())
        .select((ingredients::ingredient_json, ingredients::stock_milli))
        .first::<(String, i64)>(conn)
        .optional()?
        .ok_or(Error::IngredientNotFound)?;
    let mut i: Ingredient = serde_json::from_str(&json)?;
    i.stock = stock;
    if !i.active {
        return Err(Error::IngredientNotFound);
    }
    Ok(i)
}
fn activity(
    conn: &mut SqliteConnection,
    id: Uuid,
    now: i64,
    staff: &Attribution,
    detail: serde_json::Value,
) -> Result<()> {
    crate::audit::record(
        conn,
        crate::ActivityKind::Stock,
        &id.to_string(),
        now,
        staff,
        &detail,
    )
}
impl Pos {
    pub fn suppliers(&mut self, search: &str, include_inactive: bool) -> Result<Vec<Supplier>> {
        let mut q = suppliers::table.into_boxed();
        if !include_inactive {
            q = q.filter(suppliers::active.eq(true));
        }
        let search = search.trim();
        if !search.is_empty() {
            let pattern = format!(
                "%{}%",
                search
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            q = q.filter(suppliers::name.like(pattern).escape('\\'));
        }
        q.order((suppliers::name, suppliers::id))
            .limit(500)
            .select(suppliers::supplier_json)
            .load::<String>(&mut self.conn)?
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub fn save_supplier(&mut self, input: &Supplier, now: i64) -> Result<Supplier> {
        input.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Catalog))?;
            let mut s = input.clone();
            s.name = s.name.trim().into();
            s.contact = s.contact.trim().into();
            s.note = s.note.trim().into();
            if supplier(conn, s.id)?.as_ref() == Some(&s) {
                return Ok(s);
            }
            diesel::insert_into(suppliers::table)
                .values((
                    suppliers::id.eq(s.id.to_string()),
                    suppliers::name.eq(&s.name),
                    suppliers::active.eq(s.active),
                    suppliers::supplier_json.eq(serde_json::to_string(&s)?),
                    suppliers::updated_at.eq(now),
                ))
                .on_conflict(suppliers::id)
                .do_update()
                .set((
                    suppliers::name.eq(&s.name),
                    suppliers::active.eq(s.active),
                    suppliers::supplier_json.eq(serde_json::to_string(&s)?),
                    suppliers::updated_at.eq(now),
                ))
                .execute(conn)?;
            activity(
                conn,
                s.id,
                now,
                &staff,
                serde_json::json!({"action":"supplier","supplier":s}),
            )?;
            Ok(s)
        })
    }
    pub fn create_purchase_order(
        &mut self,
        request: &PurchaseOrderRequest,
        now: i64,
    ) -> Result<PurchaseOrder> {
        request.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            if let Some(json) = purchase_orders::table
                .find(request.id.to_string())
                .select(purchase_orders::request_json)
                .first::<String>(conn)
                .optional()?
            {
                let prior: PurchaseOrderRequest = serde_json::from_str(&json)?;
                return if prior == *request {
                    order(conn, request.id)
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Catalog))?;
            let supplier = supplier(conn, request.supplier_id)?
                .filter(|s| s.active)
                .ok_or(Error::SupplierNotFound)?;
            let mut items = Vec::new();
            for item in &request.items {
                let (name, unit) = match item.target {
                    PurchaseTarget::Product(id) => {
                        let p = crate::catalog::find_product(conn, id)?;
                        if !p.active {
                            return Err(Error::ProductUnavailable);
                        }
                        if p.stock.is_none() {
                            return Err(Error::StockNotTracked);
                        }
                        (p.name, "ชิ้น".into())
                    }
                    PurchaseTarget::Ingredient(id) => {
                        let i = ingredient(conn, id)?;
                        (i.name, i.unit)
                    }
                };
                items.push(PurchaseLine {
                    target: item.target,
                    name,
                    unit,
                    quantity: item.quantity,
                    received: 0,
                    unit_cost: item.unit_cost,
                });
            }
            let o = PurchaseOrder {
                id: request.id,
                supplier,
                items,
                note: request.note.clone(),
                created_at: now,
                revision: 0,
                cancelled: false,
                received_cost: Money::ZERO,
                cancellation: None,
                staff,
            };
            diesel::insert_into(purchase_orders::table)
                .values((
                    purchase_orders::id.eq(o.id.to_string()),
                    purchase_orders::supplier_id.eq(o.supplier.id.to_string()),
                    purchase_orders::created_at.eq(now),
                    purchase_orders::revision.eq(0_i64),
                    purchase_orders::order_json.eq(serde_json::to_string(&o)?),
                    purchase_orders::request_json.eq(serde_json::to_string(request)?),
                ))
                .execute(conn)?;
            activity(
                conn,
                o.id,
                now,
                &o.staff,
                serde_json::json!({"action":"purchase_order","order":o}),
            )?;
            Ok(o)
        })
    }
    pub fn purchase_order(&mut self, id: Uuid) -> Result<PurchaseOrder> {
        order(&mut self.conn, id)
    }
    pub fn purchase_orders(&mut self, offset: u32) -> Result<Vec<PurchaseOrder>> {
        purchase_orders::table
            .order((
                purchase_orders::created_at.desc(),
                purchase_orders::id.desc(),
            ))
            .limit(50)
            .offset(i64::from(offset))
            .select(purchase_orders::order_json)
            .load::<String>(&mut self.conn)?
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub fn purchase_receipts(
        &mut self,
        order_id: Uuid,
        offset: u32,
    ) -> Result<Vec<PurchaseReceipt>> {
        purchase_receipts::table
            .filter(purchase_receipts::order_id.eq(order_id.to_string()))
            .order((
                purchase_receipts::created_at.desc(),
                purchase_receipts::id.desc(),
            ))
            .limit(50)
            .offset(i64::from(offset))
            .select(purchase_receipts::receipt_json)
            .load::<String>(&mut self.conn)?
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub fn receive_purchase_order(
        &mut self,
        request: &ReceivePurchaseRequest,
        now: i64,
    ) -> Result<PurchaseReceipt> {
        request.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            if let Some(json) = purchase_receipts::table
                .find(request.id.to_string())
                .select(purchase_receipts::receipt_json)
                .first::<String>(conn)
                .optional()?
            {
                let prior: PurchaseReceipt = serde_json::from_str(&json)?;
                return if prior.request == *request {
                    Ok(prior)
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Stock))?;
            let mut o = order(conn, request.order_id)?;
            if o.revision != request.revision {
                return Err(Error::PurchaseOrderChanged);
            }
            if matches!(
                o.status(),
                PurchaseStatus::Cancelled | PurchaseStatus::Received
            ) {
                return Err(Error::PurchaseOrderClosed);
            }
            let mut lines = Vec::new();
            let mut total = Money::ZERO;
            for input in &request.items {
                let l = o
                    .items
                    .get_mut(input.line)
                    .ok_or(DomainError::InvalidPurchase)?;
                l.target.validate_quantity(input.quantity)?;
                let received_before = l.received;
                l.received = l
                    .received
                    .checked_add(input.quantity)
                    .ok_or(DomainError::Overflow)?;
                if l.received > l.quantity {
                    return Err(Error::PurchaseExceedsOrder);
                }
                let value = purchase_receive_cost(l.unit_cost, received_before, input.quantity)?;
                total = total.checked_add(value)?;
                lines.push(PurchaseReceivedLine {
                    received_before,
                    line: input.line,
                    target: l.target,
                    name: l.name.clone(),
                    unit: l.unit.clone(),
                    quantity: input.quantity,
                    unit_cost: l.unit_cost,
                    total: value,
                });
            }
            for l in &lines {
                let reason = format!("PO {} / {}", o.id, request.id);
                match l.target {
                    PurchaseTarget::Product(id) => {
                        let p = crate::catalog::find_product(conn, id)?;
                        if !p.active {
                            return Err(Error::ProductUnavailable);
                        }
                        let before = p.stock.ok_or(Error::StockNotTracked)?;
                        let added =
                            u32::try_from(l.quantity / MILLI).map_err(|_| DomainError::Overflow)?;
                        let after = before.checked_add(added).ok_or(DomainError::Overflow)?;
                        diesel::update(products::table.find(id.to_string()))
                            .set(products::stock.eq(Some(i64::from(after))))
                            .execute(conn)?;
                        crate::stock::insert(
                            conn,
                            &StockMovement {
                                id: Uuid::new_v4(),
                                product_id: id,
                                created_at: now,
                                kind: StockKind::Receive,
                                before,
                                after,
                                reason,
                                staff: staff.clone(),
                            },
                        )?;
                    }
                    PurchaseTarget::Ingredient(id) => {
                        let i = ingredient(conn, id)?;
                        if i.unit != l.unit {
                            return Err(Error::StockChanged);
                        }
                        let after = i
                            .stock
                            .checked_add(l.quantity)
                            .ok_or(DomainError::Overflow)?;
                        diesel::update(ingredients::table.find(id.to_string()))
                            .set((
                                ingredients::stock_milli.eq(after),
                                ingredients::updated_at.eq(now),
                            ))
                            .execute(conn)?;
                        let m = IngredientMovement {
                            id: Uuid::new_v4(),
                            ingredient_id: id,
                            created_at: now,
                            kind: IngredientKind::Receive,
                            before: i.stock,
                            after,
                            reason,
                            sale_id: None,
                            staff: staff.clone(),
                        };
                        diesel::insert_into(ingredient_movements::table)
                            .values((
                                ingredient_movements::id.eq(m.id.to_string()),
                                ingredient_movements::ingredient_id.eq(id.to_string()),
                                ingredient_movements::created_at.eq(now),
                                ingredient_movements::kind.eq("receive"),
                                ingredient_movements::sale_id.eq(None::<String>),
                                ingredient_movements::movement_json.eq(serde_json::to_string(&m)?),
                            ))
                            .execute(conn)?;
                    }
                }
            }
            o.revision = o.revision.checked_add(1).ok_or(DomainError::Overflow)?;
            o.received_cost = o.received_cost.checked_add(total)?;
            save_order(conn, &o)?;
            let receipt = PurchaseReceipt {
                id: request.id,
                order_id: o.id,
                request: request.clone(),
                created_at: now,
                lines,
                total,
                staff,
            };
            diesel::insert_into(purchase_receipts::table)
                .values((
                    purchase_receipts::id.eq(receipt.id.to_string()),
                    purchase_receipts::order_id.eq(o.id.to_string()),
                    purchase_receipts::created_at.eq(now),
                    purchase_receipts::receipt_json.eq(serde_json::to_string(&receipt)?),
                ))
                .execute(conn)?;
            activity(
                conn,
                o.id,
                now,
                &receipt.staff,
                serde_json::json!({"action":"purchase_receive","receipt":receipt}),
            )?;
            Ok(receipt)
        })
    }
    pub fn cancel_purchase_order(
        &mut self,
        request: &CancelPurchaseRequest,
        now: i64,
    ) -> Result<PurchaseOrder> {
        request.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            // A cancellation ID is unique across all orders, including stale retries.
            if let Some(json) = purchase_cancellations::table
                .find(request.id.to_string())
                .select(purchase_cancellations::cancellation_json)
                .first::<String>(conn)
                .optional()?
            {
                let c: PurchaseCancellation = serde_json::from_str(&json)?;
                return if c.request == *request {
                    order(conn, c.request.order_id)
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Catalog))?;
            let mut o = order(conn, request.order_id)?;
            if o.revision != request.revision {
                return Err(Error::PurchaseOrderChanged);
            }
            if matches!(
                o.status(),
                PurchaseStatus::Received | PurchaseStatus::Cancelled
            ) {
                return Err(Error::PurchaseOrderClosed);
            }
            o.revision = o.revision.checked_add(1).ok_or(DomainError::Overflow)?;
            o.cancelled = true;
            o.cancellation = Some(PurchaseCancellation {
                request: request.clone(),
                created_at: now,
                staff: staff.clone(),
            });
            save_order(conn, &o)?;
            diesel::insert_into(purchase_cancellations::table)
                .values((
                    purchase_cancellations::id.eq(request.id.to_string()),
                    purchase_cancellations::order_id.eq(o.id.to_string()),
                    purchase_cancellations::cancellation_json
                        .eq(serde_json::to_string(&o.cancellation)?),
                ))
                .execute(conn)?;
            activity(
                conn,
                o.id,
                now,
                &staff,
                serde_json::json!({"action":"purchase_cancel","cancellation":o.cancellation}),
            )?;
            Ok(o)
        })
    }
}
/// Check every purchasing row and its authoritative columns, including cumulative receipts.
pub(crate) fn validate_recovery(conn: &mut SqliteConnection) -> Result<()> {
    for (id, name, active, json) in suppliers::table
        .select((
            suppliers::id,
            suppliers::name,
            suppliers::active,
            suppliers::supplier_json,
        ))
        .load::<(String, String, bool, String)>(conn)?
    {
        let s: Supplier = serde_json::from_str(&json)?;
        s.validate()?;
        if s.id.to_string() != id || s.name != name || s.active != active {
            return Err(Error::CorruptData);
        }
    }
    let mut counts = std::collections::HashMap::<Uuid, (Vec<i64>, Money, i64)>::new();
    for (id, order_id, created, json) in purchase_receipts::table
        .order((
            diesel::dsl::sql::<diesel::sql_types::BigInt>(
                "json_extract(receipt_json, '$.request.revision')",
            ),
            purchase_receipts::id,
        ))
        .select((
            purchase_receipts::id,
            purchase_receipts::order_id,
            purchase_receipts::created_at,
            purchase_receipts::receipt_json,
        ))
        .load::<(String, String, i64, String)>(conn)?
    {
        let r: PurchaseReceipt = serde_json::from_str(&json)?;
        r.validate()?;
        if r.id.to_string() != id || r.order_id.to_string() != order_id || r.created_at != created {
            return Err(Error::CorruptData);
        }
        let o = order(conn, r.order_id)?;
        let c = counts
            .entry(o.id)
            .or_insert_with(|| (vec![0; o.items.len()], Money::ZERO, 0));
        if c.2 != r.request.revision {
            return Err(Error::CorruptData);
        }
        for l in &r.lines {
            let p = o.items.get(l.line).ok_or(Error::CorruptData)?;
            if p.target != l.target
                || p.name != l.name
                || p.unit != l.unit
                || p.unit_cost != l.unit_cost
            {
                return Err(Error::CorruptData);
            }
            if c.0[l.line] != l.received_before {
                return Err(Error::CorruptData);
            }
            c.0[l.line] = c.0[l.line]
                .checked_add(l.quantity)
                .ok_or(Error::CorruptData)?;
        }
        c.1 = c.1.checked_add(r.total)?;
        c.2 = c.2.checked_add(1).ok_or(Error::CorruptData)?;
    }
    for (id, supplier_id, created, revision, json, request_json) in purchase_orders::table
        .select((
            purchase_orders::id,
            purchase_orders::supplier_id,
            purchase_orders::created_at,
            purchase_orders::revision,
            purchase_orders::order_json,
            purchase_orders::request_json,
        ))
        .load::<(String, String, i64, i64, String, String)>(conn)?
    {
        let o: PurchaseOrder = serde_json::from_str(&json)?;
        o.validate()?;
        let req: PurchaseOrderRequest = serde_json::from_str(&request_json)?;
        req.validate()?;
        if o.id.to_string() != id
            || o.supplier.id.to_string() != supplier_id
            || o.created_at != created
            || o.revision != revision
            || req.id != o.id
            || req.supplier_id != o.supplier.id
            || req.note != o.note
            || req.items.len() != o.items.len()
        {
            return Err(Error::CorruptData);
        }
        for (r, l) in req.items.iter().zip(&o.items) {
            if r.target != l.target || r.quantity != l.quantity || r.unit_cost != l.unit_cost {
                return Err(Error::CorruptData);
            }
        }
        let (quantities, total, receipts) = counts
            .remove(&o.id)
            .unwrap_or_else(|| (vec![0; o.items.len()], Money::ZERO, 0));
        if total != o.received_cost
            || o.revision != receipts + i64::from(o.cancelled)
            || o.items.iter().zip(quantities).any(|(l, q)| l.received != q)
        {
            return Err(Error::CorruptData);
        }
        let c = purchase_cancellations::table
            .filter(purchase_cancellations::order_id.eq(&id))
            .select(purchase_cancellations::cancellation_json)
            .first::<String>(conn)
            .optional()?
            .map(|j| serde_json::from_str::<PurchaseCancellation>(&j))
            .transpose()?;
        if c != o.cancellation {
            return Err(Error::CorruptData);
        }
    }
    for (id, order_id, json) in purchase_cancellations::table
        .select((
            purchase_cancellations::id,
            purchase_cancellations::order_id,
            purchase_cancellations::cancellation_json,
        ))
        .load::<(String, String, String)>(conn)?
    {
        let c: PurchaseCancellation = serde_json::from_str(&json)?;
        c.request.validate()?;
        if c.request.id.to_string() != id
            || c.request.order_id.to_string() != order_id
            || order(conn, c.request.order_id)?.cancellation.as_ref() != Some(&c)
        {
            return Err(Error::CorruptData);
        }
    }
    Ok(())
}
