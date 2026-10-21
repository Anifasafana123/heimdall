//! Database connection pool initialisation and helpers.

use anyhow::{Context, Result};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::PgPool;
use tracing::info;

use crate::config::DatabaseConfig;

pub async fn init_pool(cfg: &DatabaseConfig) -> Result<PgPool> {
    let connect_opts = cfg
        .url
        .parse::<PgConnectOptions>()
        .context("invalid DATABASE_URL")?
        .ssl_mode(PgSslMode::Prefer);

    let pool = PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .acquire_timeout(cfg.connect_timeout)
        .idle_timeout(Some(cfg.idle_timeout))
        .max_lifetime(Some(cfg.max_lifetime))
        .test_before_acquire(true)
        .connect_with(connect_opts)
        .await
        .context("failed to connect to PostgreSQL")?;

    info!(
        max_connections = cfg.max_connections,
        "database pool ready"
    );

    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    info!("running database migrations");
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("database migration failed")?;
    info!("migrations complete");
    Ok(())
}

pub async fn health_check(pool: &PgPool) -> Result<(), String> {
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| {
            tracing::error!(error = %e, "database health check failed");
            format!("database unreachable: {e}")
        })
}

#[derive(Debug, serde::Serialize)]
pub struct PoolStats {
    pub size: u32,
    pub idle: usize,
    pub max:  u32,
}

pub fn pool_stats(pool: &PgPool) -> PoolStats {
    PoolStats {
        size: pool.size(),
        idle: pool.num_idle(),
        max:  pool.options().get_max_connections(),
    }
}
