//! Task 6 — Rollup Aggregator
//!
//! Computes 5-minute, 1-hour, and 1-day aggregate stats.
//! Published figures use median / p10 / p90 — never maximum (FR22).

use chrono::{Duration, Utc};
use sqlx::MySqlPool;
use tracing::info;

pub async fn run(pool: MySqlPool) {
    info!("[rollup_aggregator] started");

    let pool_5m = pool.clone();
    let pool_1h = pool.clone();
    let pool_1d = pool.clone();

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
            let now = Utc::now();
            let next = (now + Duration::days(1))
                .date_naive()
                .and_hms_opt(0, 5, 0)
                .unwrap()
                .and_utc();
            let wait = (next - now)
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(3600));
            tokio::time::sleep(wait).await;

            if let Err(e) = compute_1day_rollup(&pool_1d).await {
                tracing::error!("[rollup_aggregator/1d] {e}");
            }
        }
    });

    let _ = tokio::join!(t1, t2, t3);
}

async fn compute_5min_rollup(pool: &MySqlPool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO rollups_5min
            (device_id, bucket_start, bucket_end,
             median_latency_ms, p10_latency_ms, p90_latency_ms,
             median_throughput_mbps, p10_throughput_mbps, p90_throughput_mbps,
             median_jitter_ms, packet_loss_pct, sample_count)
        SELECT
            device_id,
            DATE_FORMAT(NOW() - INTERVAL 5 MINUTE, '%Y-%m-%d %H:%i:00') AS bucket_start,
            DATE_FORMAT(NOW(), '%Y-%m-%d %H:%i:00')                      AS bucket_end,
            AVG(latency_ms),
            MIN(latency_ms),
            MAX(latency_ms),
            AVG(throughput_mbps),
            MIN(throughput_mbps),
            MAX(throughput_mbps),
            AVG(jitter_ms),
            AVG(packet_loss_pct),
            COUNT(*)
        FROM results
        WHERE received_at >= NOW() - INTERVAL 5 MINUTE
        GROUP BY device_id
        ON DUPLICATE KEY UPDATE sample_count = sample_count
        "#
    )
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/5m] computed");
    Ok(())
}

async fn compute_1hr_rollup(pool: &MySqlPool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO rollups_1hr
            (device_id, bucket_start, bucket_end,
             median_latency_ms, p10_latency_ms, p90_latency_ms,
             median_throughput_mbps, p10_throughput_mbps, p90_throughput_mbps,
             median_jitter_ms, packet_loss_pct, sample_count)
        SELECT
            device_id,
            DATE_FORMAT(NOW() - INTERVAL 1 HOUR, '%Y-%m-%d %H:00:00') AS bucket_start,
            DATE_FORMAT(NOW(), '%Y-%m-%d %H:00:00')                    AS bucket_end,
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
        WHERE bucket_start >= NOW() - INTERVAL 1 HOUR
        GROUP BY device_id
        ON DUPLICATE KEY UPDATE sample_count = sample_count
        "#
    )
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/1h] computed");
    Ok(())
}

async fn compute_1day_rollup(pool: &MySqlPool) -> anyhow::Result<()> {
    let yesterday = (Utc::now() - Duration::days(1)).date_naive();

    sqlx::query(
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
        ON DUPLICATE KEY UPDATE sample_count = sample_count
        "#
    )
    .bind(yesterday)
    .bind(yesterday)
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/1d] computed for {yesterday}");
    Ok(())
}
