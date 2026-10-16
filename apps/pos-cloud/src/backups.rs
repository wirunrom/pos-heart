//! Encrypted database backups uploaded by the hub; owners list and download
//! them. The server checks only the container's magic: it has no key.
use crate::{ApiError, ApiResult, AppState, auth::Owner, hub::Hub, now};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::header,
    response::IntoResponse,
};
use uuid::Uuid;

/// Largest backup accepted (the shop's whole database, encrypted).
pub const MAX_BACKUP: usize = 512 * 1024 * 1024;
/// Backups kept per shop (older ones are deleted on upload).
const KEEP: i64 = 14;
const MAGIC: &[u8] = b"PHCLOUD1";

pub async fn upload(
    State(state): State<AppState>,
    hub: Hub,
    body: Bytes,
) -> ApiResult<Json<serde_json::Value>> {
    if !body.starts_with(MAGIC) {
        return Err(ApiError::bad("not an encrypted POS Heart backup"));
    }
    let id = Uuid::new_v4();
    let created = now();
    let size = i64::try_from(body.len()).unwrap_or(i64::MAX);
    let mut db = state.db().await?;
    let tx = db.transaction().await?;
    tx.execute(
        "INSERT INTO backups (id, shop_id, created_at, size, data) VALUES ($1, $2, $3, $4, $5)",
        &[&id, &hub.shop, &created, &size, &body.as_ref()],
    )
    .await?;
    tx.execute(
        "DELETE FROM backups WHERE shop_id = $1 AND id NOT IN
           (SELECT id FROM backups WHERE shop_id = $1 ORDER BY created_at DESC LIMIT $2)",
        &[&hub.shop, &KEEP],
    )
    .await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({ "id": id, "created_at": created })))
}

pub async fn list(
    State(state): State<AppState>,
    owner: Owner,
    Path(shop): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    owner.shop(&state, shop).await?;
    let rows = state
        .db()
        .await?
        .query(
            "SELECT id, created_at, size FROM backups WHERE shop_id = $1 ORDER BY created_at DESC",
            &[&shop],
        )
        .await?;
    Ok(Json(serde_json::json!(
        rows.iter()
            .map(|r| serde_json::json!({
                "id": r.get::<_, Uuid>(0),
                "created_at": r.get::<_, i64>(1),
                "size": r.get::<_, i64>(2),
            }))
            .collect::<Vec<_>>()
    )))
}

pub async fn download(
    State(state): State<AppState>,
    owner: Owner,
    Path((shop, backup)): Path<(Uuid, Uuid)>,
) -> ApiResult<impl IntoResponse> {
    owner.shop(&state, shop).await?;
    let row = state
        .db()
        .await?
        .query_opt(
            "SELECT created_at, data FROM backups WHERE shop_id = $1 AND id = $2",
            &[&shop, &backup],
        )
        .await?
        .ok_or_else(ApiError::not_found)?;
    let created: i64 = row.get(0);
    let data: Vec<u8> = row.get(1);
    let date = time::OffsetDateTime::from_unix_timestamp(created)
        .map(|t| t.to_offset(time::macros::offset!(+7)).date().to_string())
        .unwrap_or_default();
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"pos-heart-{date}.phcloud\""),
            ),
        ],
        data,
    ))
}
