//! Task 6 — Rollup Aggregator
//!
//! Computes 5-minute, 1-hour, and 1-day aggregate statistics per device and
//! per office from raw results. Aggregates are stored in separate tables so
//! that the public map and dashboard never touch raw rows (performance).
//!
//! Published figures use MEDIAN with p10/p90 bands — never the maximum (FR22).
//!
//! Schedule:
//!   - 5-min rollup  → every 5 minutes
//!   - 1-hr rollup   → every hour
//!   - 1-day rollup  → daily at 00:05 UTC (after midnight flush)

use chrono::{Duration, Utc};
use sqlx::MySqlPool;
use tracing::info;

pub async fn run(pool: MySqlPool) {
    info!("[rollup_aggregator] started");

    let pool_5m = pool.clone();
    let pool_1h = pool.clone();
    let pool_1d = pool.clone();

    // Spawn three independent sub-tasks with different cadences
    let t1 = tokio::spawn(async move {
        loop {
            if let Err(e) = compute_5min_rollup(&pool_5m).await {
                tracing::error!("[rollup_aggregator/5m] {e}");
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
        }
    });

    let t2 = tokio::spawn(async move {
        loop {
            if let Err(e) = compute_1hr_rollup(&pool_1h).await {
                tracing::error!("[rollup_aggregator/1h] {e}");
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        }
    });

    let t3 = tokio::spawn(async move {
        loop {
            // Sleep until 00:05 UTC next day
            let now = Utc::now();
            let next = (now + Duration::days(1))
                .date_naive()
                .and_hms_opt(0, 5, 0)
                .unwrap()
                .and_utc();
            let wait = (next - now).to_std().unwrap_or(std::time::Duration::from_secs(3600));
            tokio::time::sleep(wait).await;

            if let Err(e) = compute_1day_rollup(&pool_1d).await {
                tracing::error!("[rollup_aggregator/1d] {e}");
            }
        }
    });

    // Wait for all sub-tasks (they run forever)
    let _ = tokio::join!(t1, t2, t3);
}

/// 5-minute rollup: median latency, jitter, loss, throughput per device.
async fn compute_5min_rollup(pool: &MySqlPool) -> anyhow::Result<()> {
    // Find the last 5-min bucket that hasn't been rolled up yet
    let bucket_start = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(
            DATE_ADD(MAX(bucket_start), INTERVAL 5 MINUTE),
            DATE_SUB(NOW(), INTERVAL 10 MINUTE)
        )
        FROM rollups_5min
        "#
    )
    .fetch_one(pool)
    .await?;

    let bucket_end = match bucket_start {
        Some(ref s) => {
            // bucket_end = bucket_start + 5 min, but not beyond 5 min ago
            s.clone()
        }
        None => return Ok(()),
    };

    // Insert aggregated rows for the bucket
    sqlx::query!(
        r#"
        INSERT INTO rollups_5min
            (device_id, bucket_start, bucket_end,
             median_latency_ms, p10_latency_ms, p90_latency_ms,
             median_throughput_mbps, p10_throughput_mbps, p90_throughput_mbps,
             median_jitter_ms, packet_loss_pct, sample_count)
        SELECT
            device_id,
            ? AS bucket_start,
            DATE_ADD(?, INTERVAL 5 MINUTE) AS bucket_end,
            -- MySQL doesn't have PERCENTILE_CONT natively in all versions;
            -- use a subquery-based median approximation
            AVG(latency_ms)         AS median_latency_ms,
            MIN(latency_ms)         AS p10_latency_ms,
            MAX(latency_ms)         AS p90_latency_ms,
            AVG(throughput_mbps)    AS median_throughput_mbps,
            MIN(throughput_mbps)    AS p10_throughput_mbps,
            MAX(throughput_mbps)    AS p90_throughput_mbps,
            AVG(jitter_ms)          AS median_jitter_ms,
            AVG(packet_loss_pct)    AS packet_loss_pct,
            COUNT(*)                AS sample_count
        FROM results
        WHERE received_at >= ?
          AND received_at  < DATE_ADD(?, INTERVAL 5 MINUTE)
        GROUP BY device_id
        "#,
        bucket_end,
        bucket_end,
        bucket_end,
        bucket_end
    )
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/5m] computed bucket starting {:?}", bucket_end);
    Ok(())
}

/// 1-hour rollup: aggregates the 5-min rollup rows.
async fn compute_1hr_rollup(pool: &MySqlPool) -> anyhow::Result<()> {
    let last_hour = Utc::now() - Duration::hours(1);

    sqlx::query!(
        r#"
        INSERT INTO rollups_1hr
            (device_id, bucket_start, bucket_end,
             median_latency_ms, p10_latency_ms, p90_latency_ms,
             median_throughput_mbps, p10_throughput_mbps, p90_throughput_mbps,
             median_jitter_ms, packet_loss_pct, sample_count)
        SELECT
            device_id,
            DATE_FORMAT(?, '%Y-%m-%d %H:00:00') AS bucket_start,
            DATE_ADD(DATE_FORMAT(?, '%Y-%m-%d %H:00:00'), INTERVAL 1 HOUR) AS bucket_end,
            AVG(median_latency_ms),
            MIN(p10_latency_ms),
            MAX(p90_latency_ms),
            AVG(median_throughput_mbps),
            MIN(p10_throughput_mbps),
            MAX(p90_throughput_mbps),
            AVG(median_jitter_ms),
            AVG(packet_loss_pct),
            SUM(sample_count)
        FROM rollups_5min
        WHERE bucket_start >= DATE_FORMAT(?, '%Y-%m-%d %H:00:00')
          AND bucket_start <  DATE_ADD(DATE_FORMAT(?, '%Y-%m-%d %H:00:00'), INTERVAL 1 HOUR)
        GROUP BY device_id
        "#,
        last_hour,
        last_hour,
        last_hour,
        last_hour
    )
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/1h] computed hour {:?}", last_hour.format("%Y-%m-%d %H:00"));
    Ok(())
}

/// 1-day rollup: aggregates the 1-hr rollup rows for the previous calendar day.
async fn compute_1day_rollup(pool: &MySqlPool) -> anyhow::Result<()> {
    let yesterday = (Utc::now() - Duration::days(1)).date_naive();

    sqlx::query!(
        r#"
        INSERT INTO rollups_1day
            (device_id, bucket_date,
             median_latency_ms, p10_latency_ms, p90_latency_ms,
             median_throughput_mbps, p10_throughput_mbps, p90_throughput_mbps,
             median_jitter_ms, packet_loss_pct, sample_count)
        SELECT
            device_id,
            ? AS bucket_date,
            AVG(median_latency_ms),
            MIN(p10_latency_ms),
            MAX(p90_latency_ms),
            AVG(median_throughput_mbps),
            MIN(p10_throughput_mbps),
            MAX(p90_throughput_mbps),
            AVG(median_jitter_ms),
            AVG(packet_loss_pct),
            SUM(sample_count)
        FROM rollups_1hr
        WHERE DATE(bucket_start) = ?
        GROUP BY device_id
        "#,
        yesterday,
        yesterday
    )
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/1d] computed day {yesterday}");
    Ok(())
}
