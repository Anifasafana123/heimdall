//! Task 4 — Daily Ceiling Watcher (FR32)
//!
//! Every office has a maximum number of bytes HEIMDALL may use per day.
//! This task runs every 5 minutes and:
//!   1. Tallies today's bytes used per office from the `results` table.
//!   2. Marks offices that have hit or exceeded their ceiling as `ceiling_hit`.
//!   3. Resets the `ceiling_hit` flag at midnight UTC for each new day.
//!
//! When an office is marked `ceiling_hit`, the speedtest lease manager (Task 3)
//! refuses new speedtest leases for that office for the rest of the day.
//! The agent also checks this before starting a test (dual enforcement, FR32).

use sqlx::MySqlPool;
use tracing::{info, warn};

/// Default ceiling in bytes if the office has no explicit setting (500 MB/day).
const DEFAULT_CEILING_BYTES: i64 = 500 * 1024 * 1024;

pub async fn run(pool: MySqlPool) {
    info!("[daily_ceiling_watcher] started");

    loop {
        if let Err(e) = tick(&pool).await {
            tracing::error!("[daily_ceiling_watcher] error: {e}");
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
    }
}

async fn tick(pool: &MySqlPool) -> anyhow::Result<()> {
    reset_midnight_flags(pool).await?;
    check_ceilings(pool).await?;
    Ok(())
}

/// Reset `ceiling_hit` for offices whose last reset was on a previous UTC day.
async fn reset_midnight_flags(pool: &MySqlPool) -> anyhow::Result<()> {
    let reset = sqlx::query!(
        r#"
        UPDATE offices
        SET ceiling_hit = FALSE, ceiling_reset_at = NOW()
        WHERE ceiling_hit = TRUE
          AND DATE(ceiling_reset_at) < CURDATE()
        "#
    )
    .execute(pool)
    .await?;

    if reset.rows_affected() > 0 {
        info!(
            "[daily_ceiling_watcher] midnight reset for {} office(s)",
            reset.rows_affected()
        );
    }

    Ok(())
}

/// Check each office's usage today and mark those over their ceiling.
async fn check_ceilings(pool: &MySqlPool) -> anyhow::Result<()> {
    // Sum bytes_used today per office (from results table)
    let usages: Vec<OfficeUsage> = sqlx::query_as!(
        OfficeUsage,
        r#"
        SELECT d.office_id,
               COALESCE(SUM(r.bytes_used), 0) AS bytes_today,
               COALESCE(o.daily_ceiling_bytes, ?) AS ceiling_bytes
        FROM offices o
        LEFT JOIN devices d ON d.office_id = o.id
        LEFT JOIN results r ON r.device_id = d.device_id
                            AND DATE(r.received_at) = CURDATE()
        WHERE o.ceiling_hit = FALSE
        GROUP BY d.office_id, o.daily_ceiling_bytes
        "#,
        DEFAULT_CEILING_BYTES
    )
    .fetch_all(pool)
    .await?;

    for usage in usages {
        if usage.bytes_today >= usage.ceiling_bytes {
            warn!(
                "[daily_ceiling_watcher] ceiling hit — office={} used={}B ceiling={}B",
                usage.office_id, usage.bytes_today, usage.ceiling_bytes
            );

            sqlx::query!(
                "UPDATE offices SET ceiling_hit = TRUE, ceiling_reset_at = NOW() WHERE id = ?",
                usage.office_id
            )
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct OfficeUsage {
    office_id: String,
    bytes_today: i64,
    ceiling_bytes: i64,
}
