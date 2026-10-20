//! Task 2 — Anomaly Detector (SR-C3)
//!
//! Runs every 60 seconds. Scans recently accepted results for:
//!   - Throughput that exceeds the office's 99th-percentile by more than 3×
//!   - Byte-identical repeated results from the same device
//!   - Impossible physics: throughput > negotiated link speed
//!
//! Flagged rows are moved to `quarantine` and are invisible to the public
//! until a maintainer reviews them. The original row stays in `results`
//! (append-only); a quarantine row references it.

use sqlx::MySqlPool;
use tracing::{info, warn};

/// A result row from the DB, enough for anomaly checks.
#[derive(Debug, sqlx::FromRow)]
struct ResultRow {
    id: u64,
    device_id: String,
    seq: i64,
    /// Throughput reported by the agent in Mbps (NULL if not a speed result)
    throughput_mbps: Option<f64>,
    /// Negotiated link speed in Mbps from the agent
    link_speed_mbps: Option<f64>,
    /// Raw payload hash (SHA-256 hex) for duplicate detection
    payload_hash: String,
}

pub async fn run(pool: MySqlPool) {
    info!("[anomaly_detector] started");

    loop {
        if let Err(e) = scan(&pool).await {
            tracing::error!("[anomaly_detector] error: {e}");
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

async fn scan(pool: &MySqlPool) -> anyhow::Result<()> {
    // Only examine results from the last 10 minutes that haven't been checked yet
    let rows: Vec<ResultRow> = sqlx::query_as!(
        ResultRow,
        r#"
        SELECT r.id, r.device_id, r.seq,
               r.throughput_mbps, r.link_speed_mbps, r.payload_hash
        FROM results r
        WHERE r.anomaly_checked = FALSE
          AND r.received_at >= NOW() - INTERVAL 10 MINUTE
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

        // Check 1 — impossible physics: throughput > negotiated link speed
        if let (Some(tp), Some(link)) = (row.throughput_mbps, row.link_speed_mbps) {
            if tp > link * 1.05 {
                // 5 % tolerance for measurement rounding
                reasons.push("throughput_exceeds_link_speed");
            }
        }

        // Check 2 — throughput exceeds 3× the office p99
        if let Some(tp) = row.throughput_mbps {
            let p99: Option<f64> = sqlx::query_scalar!(
                r#"
                SELECT PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY throughput_mbps)
                FROM results
                WHERE device_id IN (
                    SELECT device_id FROM devices
                    WHERE office_id = (
                        SELECT office_id FROM devices WHERE device_id = ?
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

        // Check 3 — byte-identical payload from the same device in the last 24 h
        let duplicate_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) FROM results
            WHERE device_id = ?
              AND payload_hash = ?
              AND id != ?
              AND received_at >= NOW() - INTERVAL 24 HOUR
            "#,
            row.device_id,
            row.payload_hash,
            row.id
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        if duplicate_count > 0 {
            reasons.push("duplicate_payload");
        }

        if !reasons.is_empty() {
            let reason_str = reasons.join(",");
            warn!(
                "[anomaly_detector] quarantining result id={} device={} seq={} reason={}",
                row.id, row.device_id, row.seq, reason_str
            );
            quarantine(pool, row.id, &reason_str).await?;
        }

        // Mark as checked regardless
        sqlx::query!(
            "UPDATE results SET anomaly_checked = TRUE WHERE id = ?",
            row.id
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Insert a quarantine record referencing the original result row.
/// The original stays in `results` (append-only); the quarantine row hides it
/// from the public view until a maintainer reviews it.
async fn quarantine(pool: &MySqlPool, result_id: u64, reason: &str) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO quarantine (result_id, reason, quarantined_at, reviewed)
        VALUES (?, ?, NOW(), FALSE)
        "#,
        result_id,
        reason
    )
    .execute(pool)
    .await?;

    Ok(())
}
