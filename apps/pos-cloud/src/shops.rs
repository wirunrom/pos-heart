//! Owner side: shops, linking the main register, the uploaded catalog and
//! menu edits waiting for the shop.
use crate::{ApiError, ApiResult, AppState, auth::Owner, now};
use axum::{
    Json,
    extract::{Path, State},
    http::header,
    response::IntoResponse,
};
use serde::Deserialize;
use uuid::Uuid;

/// Link codes are valid 10 minutes and used once.
const LINK_SECS: i64 = 600;
/// No 0/O or 1/I: codes are read off a screen and typed.
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub async fn dashboard_page() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'self'; style-src 'self'; script-src 'self'; img-src 'self' data:",
            ),
        ],
        include_str!("../web/index.html"),
    )
}
pub async fn dashboard_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/dashboard.js"),
    )
}
pub async fn dashboard_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../web/dashboard.css"),
    )
}

#[derive(Deserialize)]
pub struct NewShop {
    name: String,
}
pub async fn create(
    State(state): State<AppState>,
    owner: Owner,
    Json(body): Json<NewShop>,
) -> ApiResult<Json<serde_json::Value>> {
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::bad("shop name must have 1–80 characters"));
    }
    let id = Uuid::new_v4();
    state
        .db()
        .await?
        .execute(
            "INSERT INTO shops (id, owner_id, name, created_at) VALUES ($1, $2, $3, $4)",
            &[&id, &owner.id, &name, &now()],
        )
        .await?;
    Ok(Json(serde_json::json!({ "id": id, "name": name })))
}

/// The owner's shops with their main register and pending edits.
pub async fn list(
    State(state): State<AppState>,
    owner: Owner,
) -> ApiResult<Json<serde_json::Value>> {
    let rows = state
        .db()
        .await?
        .query(
            "SELECT s.id, s.name, h.name, h.linked_at, h.last_seen_at,
                    (SELECT COUNT(*) FROM changes c WHERE c.shop_id = s.id AND c.applied_at IS NULL)
             FROM shops s
             LEFT JOIN hubs h ON h.shop_id = s.id AND h.revoked_at IS NULL
             WHERE s.owner_id = $1 ORDER BY s.created_at",
            &[&owner.id],
        )
        .await?;
    Ok(Json(serde_json::json!(
        rows.iter()
            .map(|r| {
                let hub: Option<String> = r.get(2);
                let (linked, seen): (Option<i64>, Option<i64>) = (r.get(3), r.get(4));
                serde_json::json!({
                    "id": r.get::<_, Uuid>(0),
                    "name": r.get::<_, String>(1),
                    "hub": hub.map(|h| serde_json::json!({ "name": h, "linked_at": linked, "last_seen_at": seen })),
                    "pending_changes": r.get::<_, i64>(5),
                })
            })
            .collect::<Vec<_>>()
    )))
}

pub async fn link_code(
    State(state): State<AppState>,
    owner: Owner,
    Path(shop): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    owner.shop(&state, shop).await?;
    let bytes = *Uuid::new_v4().as_bytes();
    let code: String = bytes[..8]
        .iter()
        .map(|b| ALPHABET[usize::from(b % 32)] as char)
        .collect();
    let expires_at = now() + LINK_SECS;
    let db = state.db().await?;
    db.execute(
        "DELETE FROM link_codes WHERE shop_id = $1 OR expires_at < $2",
        &[&shop, &now()],
    )
    .await?;
    db.execute(
        "INSERT INTO link_codes (code, shop_id, expires_at) VALUES ($1, $2, $3)",
        &[&code, &shop, &expires_at],
    )
    .await?;
    Ok(Json(
        serde_json::json!({ "code": code, "expires_at": expires_at }),
    ))
}

/// The shop's catalog as last uploaded (archived products included).
pub async fn products(
    State(state): State<AppState>,
    owner: Owner,
    Path(shop): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    owner.shop(&state, shop).await?;
    let db = state.db().await?;
    let products = db
        .query(
            "SELECT data FROM records WHERE shop_id = $1 AND kind = 'product' AND data IS NOT NULL
             ORDER BY data->>'category', data->>'name'",
            &[&shop],
        )
        .await?;
    let categories = db
        .query(
            "SELECT data->'name' FROM records
             WHERE shop_id = $1 AND kind = 'category' AND data IS NOT NULL
             ORDER BY (data->>'position')::BIGINT",
            &[&shop],
        )
        .await?;
    Ok(Json(serde_json::json!({
        "products": products.iter().map(|r| r.get::<_, serde_json::Value>(0)).collect::<Vec<_>>(),
        "categories": categories.iter().map(|r| r.get::<_, serde_json::Value>(0)).collect::<Vec<_>>(),
    })))
}

/// A menu edit for the shop to apply: `kind` "product", `id` the product
/// (a new UUID adds one), `patch` the fields to change.
#[derive(Deserialize)]
pub struct NewChange {
    kind: String,
    id: Uuid,
    patch: serde_json::Map<String, serde_json::Value>,
}
/// Fields an owner may change from the cloud, with their JSON type check.
fn valid_patch(patch: &serde_json::Map<String, serde_json::Value>) -> bool {
    !patch.is_empty()
        && patch.iter().all(|(k, v)| match k.as_str() {
            "name" | "sku" | "category" => v.as_str().is_some_and(|s| s.chars().count() <= 80),
            "price" => v
                .as_i64()
                .is_some_and(|n| (0..=10_000_000_000).contains(&n)),
            "on_sale" | "active" => v.is_boolean(),
            _ => false,
        })
}
pub async fn add_change(
    State(state): State<AppState>,
    owner: Owner,
    Path(shop): Path<Uuid>,
    Json(body): Json<NewChange>,
) -> ApiResult<Json<serde_json::Value>> {
    owner.shop(&state, shop).await?;
    if body.kind != "product" || !valid_patch(&body.patch) {
        return Err(ApiError::bad(
            "only product name, sku, category, price (satang), on_sale and active can change",
        ));
    }
    let seq: i64 = state
        .db()
        .await?
        .query_one(
            "INSERT INTO changes (shop_id, kind, record_id, patch, created_by, created_at)
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING seq",
            &[
                &shop,
                &body.kind,
                &body.id.to_string(),
                &serde_json::Value::Object(body.patch),
                &owner.id,
                &now(),
            ],
        )
        .await?
        .get(0);
    Ok(Json(serde_json::json!({ "seq": seq })))
}

/// Recent menu edits and what the shop did with them.
pub async fn changes(
    State(state): State<AppState>,
    owner: Owner,
    Path(shop): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    owner.shop(&state, shop).await?;
    let rows = state
        .db()
        .await?
        .query(
            "SELECT c.seq, c.kind, c.record_id, c.patch, c.created_at, c.applied_at, c.outcome,
                    r.data->>'name'
             FROM changes c
             LEFT JOIN records r
               ON r.shop_id = c.shop_id AND r.kind = c.kind AND r.record_id = c.record_id
             WHERE c.shop_id = $1 ORDER BY c.seq DESC LIMIT 100",
            &[&shop],
        )
        .await?;
    Ok(Json(serde_json::json!(
        rows.iter()
            .map(|r| serde_json::json!({
                "seq": r.get::<_, i64>(0),
                "kind": r.get::<_, String>(1),
                "id": r.get::<_, String>(2),
                "patch": r.get::<_, serde_json::Value>(3),
                "created_at": r.get::<_, i64>(4),
                "applied_at": r.get::<_, Option<i64>>(5),
                "outcome": r.get::<_, Option<String>>(6),
                "name": r.get::<_, Option<String>>(7),
            }))
            .collect::<Vec<_>>()
    )))
}
