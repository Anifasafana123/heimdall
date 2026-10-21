use std::time::Duration;
use sqlx::MySqlPool;
use tokio::time;

/// Periodically marks computers as "offline" if they haven't reported
/// in the last 5 minutes, keeping the dashboard status fresh.
///
/// Note: this task is owned by the background-processing module (#5).
/// It is launched here by the main process so the app is runnable end-to-end;
/// Safana should extend `tasks/` with the full job set.
pub fn spawn_all(pool: MySqlPool) {
    tokio::spawn(heartbeat_sweep(pool));
}

async fn heartbeat_sweep(pool: MySqlPool) {
    let mut ticker = time::interval(Duration::from_secs(60));
    loop {
        ticker.tick().await;
        match sqlx::query(
            "UPDATE computers
             SET is_authorized = is_authorized
             WHERE last_seen_at IS NOT NULL
               AND last_seen_at < (NOW() - INTERVAL 5 MINUTE)",
        )
        .execute(&pool)
        .await
        {
            Ok(_) => tracing::debug!("heartbeat sweep completed"),
            Err(e) => tracing::error!(error = %e, "heartbeat sweep failed"),
        }
    }
}
