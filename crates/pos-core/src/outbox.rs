//! Cloud outbox (multi-device phase 2). Triggers from migration 026 queue a
//! pointer row (`kind`, `record_id`) in the same transaction as every write
//! the cloud keeps; this module reads pending rows with each record's current
//! JSON and deletes the rows the server acknowledged. A record that no longer
//! exists (a deleted category, option set or promotion) is sent as `None`.
use crate::{Pos, Result};
use diesel::{connection::SimpleConnection, sqlite::SqliteConnection as Conn};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Nullable, Text},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One pending record for the cloud.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutboxItem {
    /// Local order; the server keys records by (`kind`, `id`), never by seq.
    pub seq: i64,
    pub kind: String,
    pub id: String,
    /// The record as stored now; None when it was deleted.
    pub record: Option<serde_json::Value>,
}

/// Every kind the outbox carries: (kind, table, key column, JSON select).
/// Order matters for `requeue_all`: reference data before the records using it.
const KINDS: &[(&str, &str, &str, &str)] = &[
    (
        "register",
        "registers",
        "id",
        "json_object('id', id, 'name', name, 'created_at', created_at, 'retired_at', retired_at)",
    ),
    (
        "category",
        "categories",
        "name",
        "json_object('name', name, 'position', position)",
    ),
    (
        "option_set",
        "option_sets",
        "id",
        "json_object('position', position, 'set', json(set_json))",
    ),
    ("product", "products", "id", ""),
    (
        "promotion",
        "promotions",
        "id",
        "json_object('uses', uses, 'promotion', json(promo_json))",
    ),
    (
        "ingredient",
        "ingredients",
        "id",
        "json_object('stock_milli', stock_milli, 'ingredient', json(ingredient_json))",
    ),
    (
        "table",
        "dining_tables",
        "id",
        "json_object('id', id, 'zone', zone, 'name', name, 'seats', seats, 'active', active, 'sort', sort)",
    ),
    // Never the PIN hash.
    (
        "staff",
        "staff",
        "id",
        "json_object('id', id, 'name', name, 'role', role, 'active', active)",
    ),
    ("supplier", "suppliers", "id", "supplier_json"),
    (
        "purchase_order",
        "purchase_orders",
        "id",
        "json_object('revision', revision, 'order', json(order_json))",
    ),
    (
        "shift",
        "shifts",
        "id",
        "json_object('register_id', register_id, 'shift', json(shift_json))",
    ),
    ("sale", "sales", "cart_id", "receipt_json"),
    ("void", "sale_voids", "sale_id", "void_json"),
    ("refund", "sale_refunds", "id", "refund_json"),
    (
        "cash_movement",
        "cash_movements",
        "id",
        "json_object('shift_id', shift_id, 'movement', json(movement_json))",
    ),
    (
        "stock_movement",
        "stock_movements",
        "id",
        "json_object('id', id, 'product_id', product_id, 'created_at', created_at, 'kind', kind, 'before', before_qty, 'after', after_qty, 'reason', reason, 'staff', json(staff_json))",
    ),
    (
        "ingredient_movement",
        "ingredient_movements",
        "id",
        "movement_json",
    ),
    (
        "tax_invoice",
        "tax_invoices",
        "id",
        "json_object('number', number, 'invoice', json(invoice_json))",
    ),
    (
        "credit_note",
        "credit_notes",
        "id",
        "json_object('number', number, 'note', json(note_json))",
    ),
    (
        "purchase_receipt",
        "purchase_receipts",
        "id",
        "receipt_json",
    ),
    (
        "activity",
        "activity_events",
        "number",
        "json_object('number', number, 'created_at', created_at, 'kind', kind, 'entity_id', entity_id, 'event', json(event_json))",
    ),
];

#[derive(QueryableByName)]
struct Pending {
    #[diesel(sql_type = BigInt)]
    seq: i64,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    record_id: String,
}
#[derive(QueryableByName)]
struct Json {
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
}
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    n: i64,
}

fn record(conn: &mut Conn, kind: &str, id: &str) -> Result<Option<serde_json::Value>> {
    let Some((_, table, key, select)) = KINDS.iter().find(|k| k.0 == kind) else {
        return Ok(None);
    };
    if kind == "product" {
        let Ok(id) = Uuid::parse_str(id) else {
            return Ok(None);
        };
        return crate::catalog::find_any_product(conn, id)?
            .map(|p| Ok(serde_json::to_value(p)?))
            .transpose();
    }
    // Table and column names come from the fixed list above, never from data.
    let row = diesel::sql_query(format!(
        "SELECT {select} AS value FROM {table} WHERE CAST({key} AS TEXT) = ?"
    ))
    .bind::<Text, _>(id)
    .get_result::<Json>(conn)
    .optional()?;
    match row.and_then(|r| r.value) {
        Some(json) => Ok(Some(serde_json::from_str(&json)?)),
        None => Ok(None),
    }
}

impl Pos {
    /// Records waiting for the cloud.
    pub fn outbox_len(&mut self) -> Result<u64> {
        let c = diesel::sql_query("SELECT COUNT(*) AS n FROM outbox")
            .get_result::<Count>(&mut self.conn)?;
        Ok(u64::try_from(c.n).unwrap_or(0))
    }
    /// The oldest `limit` pending records with their current JSON, read in
    /// one transaction.
    pub fn outbox_batch(&mut self, limit: u32) -> Result<Vec<OutboxItem>> {
        self.conn.transaction(|conn| {
            let rows =
                diesel::sql_query("SELECT seq, kind, record_id FROM outbox ORDER BY seq LIMIT ?")
                    .bind::<BigInt, _>(i64::from(limit))
                    .load::<Pending>(conn)?;
            rows.into_iter()
                .map(|r| {
                    Ok(OutboxItem {
                        record: record(conn, &r.kind, &r.record_id)?,
                        seq: r.seq,
                        kind: r.kind,
                        id: r.record_id,
                    })
                })
                .collect()
        })
    }
    /// Removes rows the server stored. A mutable record changed after the
    /// batch was read has a newer row (higher seq) that stays queued.
    pub fn outbox_ack(&mut self, seqs: &[i64]) -> Result<()> {
        self.conn.transaction(|conn| {
            for seq in seqs {
                diesel::sql_query("DELETE FROM outbox WHERE seq = ?")
                    .bind::<BigInt, _>(*seq)
                    .execute(conn)?;
            }
            Ok(())
        })
    }
    /// Queues every record again (first link to a cloud account, or a
    /// relink): the server keeps one copy per (kind, id), so resending is safe.
    pub fn requeue_all(&mut self) -> Result<()> {
        self.conn.immediate_transaction(requeue_all)
    }
}
pub(crate) fn requeue_all(conn: &mut Conn) -> Result<()> {
    conn.batch_execute("DELETE FROM outbox;")?;
    for (kind, table, key, _) in KINDS {
        diesel::sql_query(format!(
            "INSERT INTO outbox(kind, record_id, queued_at)
             SELECT ?, CAST({key} AS TEXT), CAST(strftime('%s','now') AS INTEGER) FROM {table}"
        ))
        .bind::<Text, _>(*kind)
        .execute(conn)?;
    }
    Ok(())
}
