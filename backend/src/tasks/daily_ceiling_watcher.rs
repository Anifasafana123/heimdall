//! Task 4 — Daily Ceiling Watcher (FR32)
//!
//! Runs every 5 minutes. Enforces per-office daily bandwidth ceiling.
//! Resets at midnight UTC. No data is deleted — ceiling_hit is a flag only.

use sqlx::PgPool;
use tracing::{info, warn};

const DEFAULT_CEILING_BYTES: i64 = 500 * 1024 * 1024; // 500 MB

pub async fn run(pool: PgPool) {
    info!("[daily_ceiling_watcher] started");
    loop {
        if let Err(e) = tick(&pool).await {
            tracing::error!("[daily_ceiling_watcher] error: {e}");
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
    }
}

async fn tick(pool: &PgPool) -> anyhow::Result<()> {
    reset_midnight_flags(pool).await?;
    check_ceilings(pool).await?;
    Ok(())
}

/// Reset ceiling_hit for offices whose last reset was on a previous UTC day.
async fn reset_midnight_flags(pool: &PgPool) -> anyhow::Result<()> {
    let result = sqlx::query!(
        r#"
        UPDATE offices
        SET ceiling_hit = FALSE, ceiling_reset_at = NOW()
        WHERE ceiling_hit = TRUE
          AND ceiling_reset_at::date < CURRENT_DATE
        "#
    )
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 {
        info!(
            "[daily_ceiling_watcher] midnight reset for {} office(s)",
            result.rows_affected()
        );
    }
    Ok(())
}

/// Mark offices that have hit their ceiling today.
async fn check_ceilings(pool: &PgPool) -> anyhow::Result<()> {
    let rows = sqlx::query!(
        r#"
        SELECT o.id AS office_id,
               COALESCE(SUM(r.bytes_used), 0)                        AS bytes_today,
               COALESCE(o.daily_ceiling_bytes, $1)                   AS ceiling_bytes
        FROM offices o
        LEFT JOIN devices d ON d.office_id = o.id
        LEFT JOIN results r ON r.device_id = d.device_id
                            AND r.received_at::date = CURRENT_DATE
        WHERE o.ceiling_hit = FALSE
        GROUP BY o.id, o.daily_ceiling_bytes
        "#,
        DEFAULT_CEILING_BYTES
    )
    .fetch_all(pool)
    .await?;

    for row in rows {
        let bytes_today = row.bytes_today.unwrap_or(0);
        let ceiling_bytes = row.ceiling_bytes.unwrap_or(DEFAULT_CEILING_BYTES);

        if bytes_today >= ceiling_bytes {
            warn!(
                "[daily_ceiling_watcher] ceiling hit — office={} used={}B ceiling={}B",
                row.office_id, bytes_today, ceiling_bytes
            );

            sqlx::query!(
                "UPDATE offices SET ceiling_hit = TRUE, ceiling_reset_at = NOW() WHERE id = $1",
                row.office_id
            )
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}
