use sqlx::PgPool;
use std::time::Duration;
use tokio::time;

/// Launch all background tasks.
/// Full implementation is owned by Safana (issue #5).
/// This stub keeps the backend runnable end-to-end.
pub fn spawn_all(pool: PgPool) {
    tokio::spawn(heartbeat_sweep(pool));
}

/// Mark computers as stale if they have not reported in the last 5 minutes.
async fn heartbeat_sweep(pool: PgPool) {
    let mut ticker = time::interval(Duration::from_secs(60));
    loop {
        ticker.tick().await;
        match sqlx::query(
            "UPDATE computers
             SET last_seen_at = last_seen_at
             WHERE last_seen_at IS NOT NULL
               AND last_seen_at < NOW() - INTERVAL '5 minutes'",
        )
        .execute(&pool)
        .await
        {
            Ok(_) => tracing::debug!("heartbeat sweep completed"),
            Err(e) => tracing::error!(error = %e, "heartbeat sweep failed"),
        }
    }
}
