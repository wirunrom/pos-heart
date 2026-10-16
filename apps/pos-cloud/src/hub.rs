//! Hub side: link with a one-time code, upload outbox batches, fetch and
//! acknowledge the owner's menu edits.
use crate::{ApiError, ApiResult, AppState, auth::bearer, hash, now, token};
use axum::{
    Json,
    extract::{FromRequestParts, State},
    http::{StatusCode, request::Parts},
};
use serde::Deserialize;
use uuid::Uuid;

/// Most records in one upload.
pub const MAX_BATCH: usize = 1000;

/// A linked main register (`Authorization: Bearer <hub id>.<token>`).
pub struct Hub {
    shop: Uuid,
}
impl FromRequestParts<AppState> for Hub {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> ApiResult<Self> {
        let (id, token) = bearer(parts)
            .and_then(|b| b.split_once('.'))
            .ok_or_else(ApiError::unauthorized)?;
        let id = Uuid::parse_str(id).map_err(|_| ApiError::unauthorized())?;
        let row = state
            .db()
            .await?
            .query_opt(
                "UPDATE hubs SET last_seen_at = $3
                 WHERE id = $1 AND token_hash = $2 AND revoked_at IS NULL RETURNING shop_id",
                &[&id, &hash(token), &now()],
            )
            .await?;
        Ok(Self {
            shop: row.ok_or_else(ApiError::unauthorized)?.get(0),
        })
    }
}

#[derive(Deserialize)]
pub struct LinkRequest {
    code: String,
    name: String,
}
/// Links the main register to the shop that created `code`. A previously
/// linked hub of that shop is revoked: one main register per shop.
pub async fn link(
    State(state): State<AppState>,
    Json(body): Json<LinkRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let code: String = body
        .code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::bad("name must have 1–80 characters"));
    }
    let mut db = state.db().await?;
    let tx = db.transaction().await?;
    let row = tx
        .query_opt(
            "DELETE FROM link_codes l USING shops s
             WHERE l.code = $1 AND l.expires_at >= $2 AND s.id = l.shop_id
             RETURNING s.id, s.name",
            &[&code, &now()],
        )
        .await?;
    let Some((shop, shop_name)) = row.map(|r| (r.get::<_, Uuid>(0), r.get::<_, String>(1))) else {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "WrongCode",
            "wrong or expired code",
        ));
    };
    tx.execute(
        "UPDATE hubs SET revoked_at = $2 WHERE shop_id = $1 AND revoked_at IS NULL",
        &[&shop, &now()],
    )
    .await?;
    let id = Uuid::new_v4();
    let token = token();
    tx.execute(
        "INSERT INTO hubs (id, shop_id, name, token_hash, linked_at) VALUES ($1, $2, $3, $4, $5)",
        &[&id, &shop, &name, &hash(&token), &now()],
    )
    .await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({
        "hub": id, "token": token, "shop": shop, "shop_name": shop_name,
    })))
}

#[derive(Deserialize)]
pub struct Item {
    seq: i64,
    kind: String,
    id: String,
    record: Option<serde_json::Value>,
}
#[derive(Deserialize)]
pub struct Batch {
    items: Vec<Item>,
}
fn valid_kind(kind: &str) -> bool {
    !kind.is_empty()
        && kind.len() <= 40
        && kind.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
}
/// Stores a batch in one transaction: one row per (kind, id), so resends
/// and restored hubs overwrite rather than duplicate. Answers the stored
/// seqs for the hub to drop from its outbox.
pub async fn upload(
    State(state): State<AppState>,
    hub: Hub,
    Json(batch): Json<Batch>,
) -> ApiResult<Json<serde_json::Value>> {
    if batch.items.len() > MAX_BATCH {
        return Err(ApiError::bad("batch too large"));
    }
    if batch
        .items
        .iter()
        .any(|i| !valid_kind(&i.kind) || i.id.is_empty() || i.id.len() > 200)
    {
        return Err(ApiError::bad("invalid record kind or id"));
    }
    let received = now();
    let mut db = state.db().await?;
    let tx = db.transaction().await?;
    let upsert = tx
        .prepare(
            "INSERT INTO records (shop_id, kind, record_id, data, occurred_at, received_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (shop_id, kind, record_id) DO UPDATE
             SET data = EXCLUDED.data, occurred_at = EXCLUDED.occurred_at,
                 received_at = EXCLUDED.received_at",
        )
        .await?;
    let mut acked = Vec::with_capacity(batch.items.len());
    for item in batch.items {
        let occurred = item
            .record
            .as_ref()
            .and_then(|r| r.get("created_at"))
            .and_then(serde_json::Value::as_i64);
        tx.execute(
            &upsert,
            &[
                &hub.shop,
                &item.kind,
                &item.id,
                &item.record,
                &occurred,
                &received,
            ],
        )
        .await?;
        acked.push(item.seq);
    }
    tx.commit().await?;
    Ok(Json(serde_json::json!({ "acked": acked })))
}

/// The owner's menu edits not yet applied, oldest first.
pub async fn changes(
    State(state): State<AppState>,
    hub: Hub,
) -> ApiResult<Json<serde_json::Value>> {
    let rows = state
        .db()
        .await?
        .query(
            "SELECT seq, kind, record_id, patch FROM changes
             WHERE shop_id = $1 AND applied_at IS NULL ORDER BY seq LIMIT 100",
            &[&hub.shop],
        )
        .await?;
    Ok(Json(serde_json::json!(
        rows.iter()
            .map(|r| serde_json::json!({
                "seq": r.get::<_, i64>(0),
                "kind": r.get::<_, String>(1),
                "id": r.get::<_, String>(2),
                "patch": r.get::<_, serde_json::Value>(3),
            }))
            .collect::<Vec<_>>()
    )))
}

#[derive(Deserialize)]
pub struct Outcome {
    seq: i64,
    /// "applied", or why the shop refused it.
    outcome: String,
}
#[derive(Deserialize)]
pub struct Acks {
    results: Vec<Outcome>,
}
pub async fn ack(
    State(state): State<AppState>,
    hub: Hub,
    Json(acks): Json<Acks>,
) -> ApiResult<StatusCode> {
    let db = state.db().await?;
    for r in acks.results {
        db.execute(
            "UPDATE changes SET applied_at = $3, outcome = $4
             WHERE shop_id = $1 AND seq = $2 AND applied_at IS NULL",
            &[
                &hub.shop,
                &r.seq,
                &now(),
                &r.outcome.chars().take(300).collect::<String>(),
            ],
        )
        .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}
