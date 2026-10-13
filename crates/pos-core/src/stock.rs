use crate::{
    Error, Pos, Result,
    schema::{products, stock_movements},
};
use diesel::prelude::*;
use pos_domain::{StockKind, StockMovement, StockRequest};
use uuid::Uuid;

pub(crate) fn insert(conn: &mut SqliteConnection, m: &StockMovement) -> Result<()> {
    diesel::insert_into(stock_movements::table)
        .values((
            stock_movements::id.eq(m.id.to_string()),
            stock_movements::product_id.eq(m.product_id.to_string()),
            stock_movements::created_at.eq(m.created_at),
            stock_movements::kind.eq(m.kind.key()),
            stock_movements::before_qty.eq(i64::from(m.before)),
            stock_movements::after_qty.eq(i64::from(m.after)),
            stock_movements::reason.eq(&m.reason),
            stock_movements::staff_json.eq(if m.staff.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&m.staff)?)
            }),
        ))
        .execute(conn)?;
    crate::audit::record(
        conn,
        crate::ActivityKind::Stock,
        &m.product_id.to_string(),
        m.created_at,
        &m.staff,
        &serde_json::json!({"action": m.kind.key(), "before": m.before, "after": m.after, "reason": m.reason}),
    )?;
    Ok(())
}
type Row = (
    String,
    String,
    i64,
    String,
    i64,
    i64,
    String,
    Option<String>,
);
fn from_row(
    (id, product_id, created_at, kind, before, after, reason, staff): Row,
) -> Result<StockMovement> {
    let parse = |s: &str| Uuid::parse_str(s).map_err(|_| Error::CorruptData);
    let qty = |n: i64| u32::try_from(n).map_err(|_| Error::CorruptData);
    Ok(StockMovement {
        id: parse(&id)?,
        product_id: parse(&product_id)?,
        created_at,
        kind: StockKind::from_key(&kind).ok_or(Error::CorruptData)?,
        before: qty(before)?,
        after: qty(after)?,
        reason,
        staff: match staff {
            Some(json) => serde_json::from_str(&json)?,
            None => pos_domain::Attribution::default(),
        },
    })
}
fn columns() -> (
    stock_movements::id,
    stock_movements::product_id,
    stock_movements::created_at,
    stock_movements::kind,
    stock_movements::before_qty,
    stock_movements::after_qty,
    stock_movements::reason,
    stock_movements::staff_json,
) {
    (
        stock_movements::id,
        stock_movements::product_id,
        stock_movements::created_at,
        stock_movements::kind,
        stock_movements::before_qty,
        stock_movements::after_qty,
        stock_movements::reason,
        stock_movements::staff_json,
    )
}
impl Pos {
    /// Receive, remove or count stock in one IMMEDIATE transaction. The same
    /// request ID returns the original movement; a different payload refuses.
    pub fn adjust_stock(&mut self, request: &StockRequest, now: i64) -> Result<StockMovement> {
        request.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            if let Some(row) = stock_movements::table
                .find(request.id.to_string())
                .select(columns())
                .first::<Row>(conn)
                .optional()?
            {
                let previous = from_row(row)?;
                let same = previous.product_id == request.product_id
                    && previous.kind == request.kind
                    && previous.reason == request.reason.trim()
                    && request.after(previous.before).ok() == Some(previous.after);
                return if same {
                    Ok(previous)
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            let staff =
                crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Stock))?;
            let product = crate::catalog::find_product(conn, request.product_id)?;
            let before = product.stock.ok_or(Error::StockNotTracked)?;
            if request.kind == StockKind::Count && request.expected.is_some_and(|e| e != before) {
                return Err(Error::StockChanged);
            }
            let after = request.after(before)?;
            diesel::update(products::table.find(request.product_id.to_string()))
                .set(products::stock.eq(Some(i64::from(after))))
                .execute(conn)?;
            let movement = StockMovement {
                id: request.id,
                product_id: request.product_id,
                created_at: now,
                kind: request.kind,
                before,
                after,
                reason: request.reason.trim().to_owned(),
                staff,
            };
            insert(conn, &movement)?;
            Ok(movement)
        })
    }
    /// Newest first, 50 per page.
    pub fn stock_movements(&mut self, product_id: Uuid, offset: u32) -> Result<Vec<StockMovement>> {
        stock_movements::table
            .filter(stock_movements::product_id.eq(product_id.to_string()))
            .order((
                stock_movements::created_at.desc(),
                stock_movements::id.desc(),
            ))
            .limit(50)
            .offset(i64::from(offset))
            .select(columns())
            .load::<Row>(&mut self.conn)?
            .into_iter()
            .map(from_row)
            .collect()
    }
}
