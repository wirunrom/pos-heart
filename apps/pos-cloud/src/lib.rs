//! POS Heart cloud server (multi-device phase 2). Shops' main registers
//! (hubs) upload their records from the outbox and fetch the owner's catalog
//! edits; owners sign in to a dashboard to read reports and edit the menu.
//! Selling never depends on this server: a hub queues while it is offline.
//!
//! JSON API under `/api/v1`, errors as `{"error": {"kind", "message"}}`,
//! bearer tokens (owner sessions, hub tokens) stored only as SHA-256.
mod auth;
mod backups;
mod hub;
mod report;
mod shops;

use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use deadpool_postgres::{Object, Pool};
use sha2::{Digest, Sha256};

/// Largest upload batch body (a batch is at most `hub::MAX_BATCH` records).
const MAX_BODY: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool,
}
impl AppState {
    pub(crate) async fn db(&self) -> ApiResult<Object> {
        Ok(self.pool.get().await?)
    }
}
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A connection pool for `url` (a `postgres://` URL).
pub fn pool(url: &str) -> Result<Pool, BoxError> {
    let config: tokio_postgres::Config = url.parse()?;
    let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
    Ok(Pool::builder(manager).max_size(16).build()?)
}

/// Creates the schema on an empty database; refuses an unknown newer one.
pub async fn migrate(pool: &Pool) -> Result<(), BoxError> {
    let mut db = pool.get().await?;
    let exists: bool = db
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'cloud_meta')",
            &[],
        )
        .await?
        .get(0);
    if !exists {
        let tx = db.transaction().await?;
        tx.batch_execute(include_str!("../migrations/001_init.sql"))
            .await?;
        tx.commit().await?;
    }
    let mut version: i32 = db
        .query_one("SELECT version FROM cloud_meta", &[])
        .await?
        .get(0);
    if version == 1 {
        let tx = db.transaction().await?;
        tx.batch_execute(include_str!("../migrations/002_backups.sql"))
            .await?;
        tx.commit().await?;
        version = 2;
    }
    if version != 2 {
        return Err(format!("unsupported cloud schema {version}").into());
    }
    Ok(())
}

/// Serves the API on `listener` until the task is dropped.
pub async fn serve(listener: tokio::net::TcpListener, state: AppState) -> std::io::Result<()> {
    axum::serve(listener, router(state)).await
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/", get(shops::dashboard_page))
        .route("/dashboard.js", get(shops::dashboard_js))
        .route("/dashboard.css", get(shops::dashboard_css))
        .route("/api/v1/accounts", post(auth::sign_up))
        .route(
            "/api/v1/sessions",
            post(auth::sign_in).delete(auth::sign_out),
        )
        .route("/api/v1/shops", get(shops::list).post(shops::create))
        .route("/api/v1/shops/{shop}/link-code", post(shops::link_code))
        .route("/api/v1/shops/{shop}/summary", get(report::summary))
        .route("/api/v1/shops/{shop}/products", get(shops::products))
        .route(
            "/api/v1/shops/{shop}/changes",
            get(shops::changes).post(shops::add_change),
        )
        .route("/api/v1/hub/link", post(hub::link))
        .route("/api/v1/hub/upload", post(hub::upload))
        .route("/api/v1/hub/changes", get(hub::changes))
        .route("/api/v1/hub/changes/ack", post(hub::ack))
        .route(
            "/api/v1/hub/backups",
            post(backups::upload).layer(DefaultBodyLimit::max(backups::MAX_BACKUP)),
        )
        .route("/api/v1/shops/{shop}/backups", get(backups::list))
        .route(
            "/api/v1/shops/{shop}/backups/{backup}",
            get(backups::download),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(state)
}

/// An API failure: HTTP status, a stable kind for code and a message.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    kind: &'static str,
    message: String,
}
impl ApiError {
    fn new(status: StatusCode, kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            kind,
            message: message.into(),
        }
    }
    fn bad(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "BadRequest", message)
    }
    fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "Unauthorized", "sign in again")
    }
    fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "NotFound", "not found")
    }
}
impl ApiError {
    fn internal(e: &dyn std::fmt::Display) -> Self {
        eprintln!("pos-cloud: database: {e}");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal",
            "server error",
        )
    }
}
impl From<tokio_postgres::Error> for ApiError {
    fn from(e: tokio_postgres::Error) -> Self {
        Self::internal(&e)
    }
}
impl From<deadpool_postgres::PoolError> for ApiError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        Self::internal(&e)
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": { "kind": self.kind, "message": self.message } })),
        )
            .into_response()
    }
}
pub type ApiResult<T> = Result<T, ApiError>;

pub(crate) fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}
pub(crate) fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
/// A random 256-bit bearer token (two v4 UUIDs, hex).
pub(crate) fn token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
