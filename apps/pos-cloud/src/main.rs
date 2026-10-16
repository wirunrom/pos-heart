//! `pos-cloud`: runs the cloud API and owner dashboard.
//! Environment: `DATABASE_URL` (PostgreSQL, required), `BIND` (default
//! `127.0.0.1:8080`). TLS is terminated by the hosting proxy in production.
use pos_cloud::{AppState, migrate, pool, router};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("pos-cloud: {e}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), pos_cloud::BoxError> {
    let url = std::env::var("DATABASE_URL").map_err(|_| "set DATABASE_URL")?;
    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let pool = pool(&url)?;
    migrate(&pool).await?;
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    eprintln!("pos-cloud: listening on http://{bind}");
    axum::serve(listener, router(AppState { pool }))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
