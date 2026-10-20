//! Task 4 — Daily Ceiling Watcher (FR32)
//!
//! Runs every 5 minutes. Tallies today's bytes used per office.
//! Marks offices that have hit their ceiling. Resets at midnight UTC.

use sqlx::MySqlPool;
use tracing::{info, warn};

const DEFAULT_CEILING_BYTES: i64 = 500 * 1024 * 1024; // 500 MB

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

async fn reset_midnight_flags(pool: &MySqlPool) -> anyhow::Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE offices
        SET ceiling_hit = FALSE, ceiling_reset_at = NOW()
        WHERE ceiling_hit = TRUE
          AND DATE(ceiling_reset_at) < CURDATE()
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

async fn check_ceilings(pool: &MySqlPool) -> anyhow::Result<()> {
    let rows = sqlx::query(
        r#"
        SELECT o.id AS office_id,
               COALESCE(SUM(r.bytes_used), 0) AS bytes_today,
               COALESCE(o.daily_ceiling_bytes, ?) AS ceiling_bytes
        FROM offices o
        LEFT JOIN devices d ON d.office_id = o.id
        LEFT JOIN results r ON r.device_id = d.device_id
                            AND DATE(r.received_at) = CURDATE()
        WHERE o.ceiling_hit = FALSE
        GROUP BY o.id, o.daily_ceiling_bytes
        "#
    )
    .bind(DEFAULT_CEILING_BYTES)
    .fetch_all(pool)
    .await?;

    for row in rows {
        use sqlx::Row;
        let office_id: String = row.try_get("office_id")?;
        let bytes_today: i64 = row.try_get("bytes_today")?;
        let ceiling_bytes: i64 = row.try_get("ceiling_bytes")?;

        if bytes_today >= ceiling_bytes {
            warn!(
                "[daily_ceiling_watcher] ceiling hit — office={} used={}B ceiling={}B",
                office_id, bytes_today, ceiling_bytes
            );

            sqlx::query(
                "UPDATE offices SET ceiling_hit = TRUE, ceiling_reset_at = NOW() WHERE id = ?"
            )
            .bind(&office_id)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}
