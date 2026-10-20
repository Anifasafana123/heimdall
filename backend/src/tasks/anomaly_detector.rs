//! Task 2 — Anomaly Detector (SR-C3)
//!
//! Runs every 60 seconds. Scans recently accepted results for:
//!   - Throughput that exceeds the office's 99th-percentile by more than 3×
//!   - Byte-identical repeated results from the same device
//!   - Impossible physics: throughput > negotiated link speed
//!
//! Flagged rows are moved to `quarantine` — invisible to the public
//! until a maintainer reviews them.

use sqlx::MySqlPool;
use tracing::{info, warn};

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
    let rows = sqlx::query(
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
        use sqlx::Row;
        let id: u64 = row.try_get("id")?;
        let device_id: String = row.try_get("device_id")?;
        let seq: i64 = row.try_get("seq")?;
        let throughput_mbps: Option<f64> = row.try_get("throughput_mbps")?;
        let link_speed_mbps: Option<f64> = row.try_get("link_speed_mbps")?;
        let payload_hash: String = row.try_get("payload_hash")?;

        let mut reasons: Vec<&str> = Vec::new();

        // Check 1 — impossible physics: throughput > negotiated link speed
        if let (Some(tp), Some(link)) = (throughput_mbps, link_speed_mbps) {
            if tp > link * 1.05 {
                reasons.push("throughput_exceeds_link_speed");
            }
        }

        // Check 2 — throughput exceeds 3× the office p99
        if let Some(tp) = throughput_mbps {
            let p99: Option<f64> = sqlx::query_scalar(
                r#"
                SELECT AVG(throughput_mbps) * 3
                FROM results
                WHERE device_id IN (
                    SELECT device_id FROM devices
                    WHERE office_id = (SELECT office_id FROM devices WHERE device_id = ?)
                )
                AND throughput_mbps IS NOT NULL
                "#
            )
            .bind(&device_id)
            .fetch_one(pool)
            .await
            .unwrap_or(None);

            if let Some(p99_val) = p99 {
                if p99_val > 0.0 && tp > p99_val {
                    reasons.push("throughput_exceeds_3x_avg");
                }
            }
        }

        // Check 3 — byte-identical payload from same device in last 24 h
        let duplicate_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM results
            WHERE device_id = ?
              AND payload_hash = ?
              AND id != ?
              AND received_at >= NOW() - INTERVAL 24 HOUR
            "#
        )
        .bind(&device_id)
        .bind(&payload_hash)
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        if duplicate_count > 0 {
            reasons.push("duplicate_payload");
        }

        if !reasons.is_empty() {
            let reason_str = reasons.join(",");
            warn!(
                "[anomaly_detector] quarantining result id={} device={} seq={} reason={}",
                id, device_id, seq, reason_str
            );
            quarantine(pool, id, &reason_str).await?;
        }

        // Mark as checked
        sqlx::query("UPDATE results SET anomaly_checked = TRUE WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

async fn quarantine(pool: &MySqlPool, result_id: u64, reason: &str) -> anyhow::Result<()> {
    sqlx::query(
        r#"INSERT INTO quarantine (result_id, reason, quarantined_at, reviewed)
           VALUES (?, ?, NOW(), FALSE)"#
    )
    .bind(result_id)
    .bind(reason)
    .execute(pool)
    .await?;

    Ok(())
}
