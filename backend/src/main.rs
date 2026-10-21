//! HEIMDALL backend — entry point.

use anyhow::Result;
use tracing::info;

mod config;
mod db;
mod error;
mod queries;
mod routes;
mod state;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = config::Config::from_env()?;
    info!(bind = %cfg.server.bind_addr(), "starting HEIMDALL backend");

    let pool = db::init_pool(&cfg.database).await?;
    db::run_migrations(&pool).await?;

    let app_state = state::AppState::new(pool);
    let app = routes::router(app_state);

    let listener = tokio::net::TcpListener::bind(cfg.server.bind_addr()).await?;
    info!(addr = %listener.local_addr()?, "listening");

    axum::serve(listener, app).await?;
    Ok(())
}
