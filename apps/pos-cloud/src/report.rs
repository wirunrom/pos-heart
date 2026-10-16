//! Owner reports from uploaded receipts, voids and refunds (Bangkok days).
//! The shop's own Reports stay authoritative; this is the remote view. All
//! amounts are satang. Sales count on their payment day, voids and refunds
//! on the day they were made, as in the desktop reports.
use crate::{ApiError, ApiResult, AppState, auth::Owner};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use pos_domain::{Sale, SaleRefund, SaleVoid};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use time::{Date, OffsetDateTime, macros::format_description, macros::offset};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct Period {
    from: String,
    to: String,
}
#[derive(Default, Serialize)]
pub struct Day {
    date: String,
    sales: i64,
    refunds: i64,
}
#[derive(Default, Serialize)]
pub struct Method {
    sales: i64,
    refunds: i64,
}
#[derive(Serialize)]
pub struct ProductLine {
    name: String,
    quantity: i64,
    amount: i64,
}
#[derive(Default, Serialize)]
pub struct Summary {
    from: String,
    to: String,
    sale_count: u64,
    void_count: u64,
    refund_count: u64,
    sales: i64,
    refunds: i64,
    net: i64,
    days: Vec<Day>,
    methods: BTreeMap<String, Method>,
    hourly: Vec<i64>,
    top_products: Vec<ProductLine>,
    /// Newest upload time of any record of the shop (how fresh this is).
    uploaded_at: Option<i64>,
}

fn date(s: &str) -> ApiResult<Date> {
    Date::parse(s, format_description!("[year]-[month]-[day]"))
        .map_err(|_| ApiError::bad("dates are YYYY-MM-DD"))
}
fn bangkok(ts: i64) -> Option<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp(ts)
        .ok()
        .map(|t| t.to_offset(offset!(+7)))
}

pub async fn summary(
    State(state): State<AppState>,
    owner: Owner,
    Path(shop): Path<Uuid>,
    Query(period): Query<Period>,
) -> ApiResult<Json<Summary>> {
    owner.shop(&state, shop).await?;
    let (first, last) = (date(&period.from)?, date(&period.to)?);
    let days = (last - first).whole_days() + 1;
    if !(1..=366).contains(&days) {
        return Err(ApiError::bad("a period is 1–366 days"));
    }
    let start = first.midnight().assume_offset(offset!(+7)).unix_timestamp();
    let end = start + days * 86_400;
    let db = state.db().await?;
    let rows = db
        .query(
            "SELECT kind, data FROM records
             WHERE shop_id = $1 AND kind IN ('sale', 'void', 'refund') AND data IS NOT NULL
               AND occurred_at >= $2 AND occurred_at < $3
             ORDER BY occurred_at",
            &[&shop, &start, &end],
        )
        .await?;
    let uploaded_at: Option<i64> = db
        .query_one(
            "SELECT MAX(received_at) FROM records WHERE shop_id = $1",
            &[&shop],
        )
        .await?
        .get(0);
    let mut s = Summary {
        from: first.to_string(),
        to: last.to_string(),
        hourly: vec![0; 24],
        uploaded_at,
        ..Summary::default()
    };
    let mut day = first;
    for _ in 0..days {
        s.days.push(Day {
            date: day.to_string(),
            ..Day::default()
        });
        day = day.next_day().unwrap_or(day);
    }
    let index = |ts: i64| usize::try_from((ts - start) / 86_400).ok();
    let mut products: HashMap<Uuid, ProductLine> = HashMap::new();
    let mut product = |id: Uuid, name: &str, quantity: i64, amount: i64| {
        let p = products.entry(id).or_insert_with(|| ProductLine {
            name: name.to_owned(),
            quantity: 0,
            amount: 0,
        });
        p.quantity += quantity;
        p.amount += amount;
    };
    // Records the server cannot read (a newer app version) are skipped.
    for row in rows {
        let (kind, data): (String, serde_json::Value) = (row.get(0), row.get(1));
        match kind.as_str() {
            "sale" => {
                let Ok(sale) = serde_json::from_value::<Sale>(data) else {
                    continue;
                };
                let total = sale.total.satang();
                s.sale_count += 1;
                s.sales += total;
                if let Some(d) = index(sale.created_at).and_then(|i| s.days.get_mut(i)) {
                    d.sales += total;
                }
                if let Some(t) = bangkok(sale.created_at) {
                    s.hourly[usize::from(t.hour())] += total;
                }
                for t in sale.paid_by() {
                    s.methods
                        .entry(t.method.key().to_owned())
                        .or_default()
                        .sales += t.amount.satang();
                }
                for line in &sale.lines {
                    product(
                        line.product_id,
                        &line.name,
                        i64::from(line.quantity),
                        line.net().satang(),
                    );
                }
            }
            "void" => {
                let Ok(v) = serde_json::from_value::<SaleVoid>(data) else {
                    continue;
                };
                let amount = v.refund.satang();
                s.void_count += 1;
                s.refunds += amount;
                if let Some(d) = index(v.created_at).and_then(|i| s.days.get_mut(i)) {
                    d.refunds += amount;
                }
                for t in v.refunded_by() {
                    s.methods
                        .entry(t.method.key().to_owned())
                        .or_default()
                        .refunds += t.amount.satang();
                }
            }
            "refund" => {
                let Ok(r) = serde_json::from_value::<SaleRefund>(data) else {
                    continue;
                };
                let amount = r.refund.satang();
                s.refund_count += 1;
                s.refunds += amount;
                if let Some(d) = index(r.created_at).and_then(|i| s.days.get_mut(i)) {
                    d.refunds += amount;
                }
                s.methods
                    .entry(r.refund_method.key().to_owned())
                    .or_default()
                    .refunds += amount;
                for line in &r.lines {
                    product(
                        line.product_id,
                        &line.name,
                        -i64::from(line.quantity),
                        -line.amount.satang(),
                    );
                }
            }
            _ => {}
        }
    }
    s.net = s.sales - s.refunds;
    let mut top: Vec<ProductLine> = products.into_values().collect();
    top.sort_by(|a, b| b.amount.cmp(&a.amount).then_with(|| a.name.cmp(&b.name)));
    top.truncate(10);
    s.top_products = top;
    Ok(Json(s))
}
