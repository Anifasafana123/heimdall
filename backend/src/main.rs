mod app;
mod auth;
mod config;
mod error;
mod handlers;
mod models;
mod tasks;
mod views;

use sqlx::mysql::MySqlPoolOptions;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if present
    let _ = dotenvy::dotenv();

    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = config::Config::from_env()?;
    tracing::info!(host = %config.host, port = config.port, "starting backend");

    // Database pool
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    // Run pending migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    let state = app::AppState {
        pool: pool.clone(),
        config: config.clone(),
    };

    // Spawn background tasks
    tasks::spawn_all(pool.clone());

    let router = app::app(state);
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on {addr}");
    axum::serve(listener, router).await?;
    Ok(())
}
