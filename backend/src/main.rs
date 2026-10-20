mod tasks;

use dotenvy::dotenv;
use sqlx::mysql::MySqlPoolOptions;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env
    dotenv().ok();

    // Set up tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "heimdall_backend=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("HEIMDALL backend starting...");

    // Connect to MySQL
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env");

    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    info!("Connected to database.");

    // Start all background tasks (Safana - Issue #5)
    tasks::start_all(pool).await;

    Ok(())
}
