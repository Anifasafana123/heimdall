//! Task 2 — Anomaly Detector (SR-C3)
//!
//! Runs every 60 seconds. Flags suspicious results to quarantine.
//! Quarantined rows stay in `results` (append-only) but are hidden from the
//! public until a maintainer reviews them.
//!
//! Checks:
//!   - Throughput > 3× office p99 (suspicious spike)
//!   - Throughput > negotiated link speed (impossible physics)
//!   - Byte-identical repeated payload from the same device in 24 h

use sqlx::PgPool;
use tracing::{info, warn};

pub async fn run(pool: PgPool) {
    info!("[anomaly_detector] started");
    loop {
        if let Err(e) = scan(&pool).await {
            tracing::error!("[anomaly_detector] error: {e}");
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

async fn scan(pool: &PgPool) -> anyhow::Result<()> {
    let rows = sqlx::query!(
        r#"
        SELECT r.id, r.device_id, r.seq,
               r.throughput_mbps, r.link_speed_mbps, r.payload_hash
        FROM results r
        WHERE r.anomaly_checked = FALSE
          AND r.received_at >= NOW() - INTERVAL '10 minutes'
        LIMIT 500
        "#
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    info!("[anomaly_detector] checking {} result(s)", rows.len());

    for row in &rows {
        let mut reasons: Vec<&str> = Vec::new();

        // Check 1 — impossible physics
        if let (Some(tp), Some(link)) = (row.throughput_mbps, row.link_speed_mbps) {
            if tp > link * 1.05 {
                reasons.push("throughput_exceeds_link_speed");
            }
        }

        // Check 2 — exceeds 3× office p99
        if let Some(tp) = row.throughput_mbps {
            let p99: Option<f64> = sqlx::query_scalar!(
                r#"
                SELECT PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY throughput_mbps)
                FROM results
                WHERE device_id IN (
                    SELECT device_id FROM devices
                    WHERE office_id = (
                        SELECT office_id FROM devices WHERE device_id = $1
                    )
                )
                AND throughput_mbps IS NOT NULL
                "#,
                row.device_id
            )
            .fetch_one(pool)
            .await
            .unwrap_or(None);

            if let Some(p99_val) = p99 {
                if p99_val > 0.0 && tp > p99_val * 3.0 {
                    reasons.push("throughput_exceeds_3x_p99");
                }
            }
        }

        // Check 3 — byte-identical payload in last 24 h
        let dup_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) FROM results
            WHERE device_id = $1
              AND payload_hash = $2
              AND id != $3
              AND received_at >= NOW() - INTERVAL '24 hours'
            "#,
            row.device_id,
            row.payload_hash,
            row.id
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        if dup_count > 0 {
            reasons.push("duplicate_payload");
        }

        if !reasons.is_empty() {
            let reason_str = reasons.join(",");
            warn!(
                "[anomaly_detector] quarantining id={} device={} seq={} reason={}",
                row.id, row.device_id, row.seq, reason_str
            );
            quarantine(pool, row.id, &reason_str).await?;
        }

        // Mark checked (result row stays — append-only)
        sqlx::query!(
            "UPDATE results SET anomaly_checked = TRUE WHERE id = $1",
            row.id
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Insert a quarantine record. The original result row is never deleted (SR-C4).
async fn quarantine(pool: &PgPool, result_id: i64, reason: &str) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO quarantine (result_id, reason, quarantined_at, reviewed)
        VALUES ($1, $2, NOW(), FALSE)
        "#,
        result_id,
        reason
    )
    .execute(pool)
    .await?;
    Ok(())
}
