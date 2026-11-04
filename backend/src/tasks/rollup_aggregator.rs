//! Task 6 — Rollup Aggregator
//!
//! Computes 5-minute, 1-hour, and 1-day aggregate stats.
//! Published figures strictly use median / p10 / p90 (FR22), never simple averages.
//! Implemented using PostgreSQL native `PERCENTILE_CONT` continuous percentiles.

use chrono::{Duration, Utc};
use sqlx::PgPool;
use tracing::info;

pub async fn run(pool: PgPool) {
    info!("[rollup_aggregator] started (PostgreSQL percentiles: p10/p50/p90)");

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

async fn compute_5min_rollup(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO rollups_5min
            (device_id, bucket_start, bucket_end,
             median_latency_ms, p10_latency_ms, p90_latency_ms,
             median_throughput_mbps, p10_throughput_mbps, p90_throughput_mbps,
             median_jitter_ms, packet_loss_pct, sample_count)
        SELECT
            device_id,
            date_trunc('minute', NOW() - INTERVAL '5 minutes') AS bucket_start,
            date_trunc('minute', NOW())                         AS bucket_end,
            percentile_cont(0.50) WITHIN GROUP (ORDER BY latency_ms) AS median_latency_ms,
            percentile_cont(0.10) WITHIN GROUP (ORDER BY latency_ms) AS p10_latency_ms,
            percentile_cont(0.90) WITHIN GROUP (ORDER BY latency_ms) AS p90_latency_ms,
            percentile_cont(0.50) WITHIN GROUP (ORDER BY throughput_mbps) AS median_throughput_mbps,
            percentile_cont(0.10) WITHIN GROUP (ORDER BY throughput_mbps) AS p10_throughput_mbps,
            percentile_cont(0.90) WITHIN GROUP (ORDER BY throughput_mbps) AS p90_throughput_mbps,
            percentile_cont(0.50) WITHIN GROUP (ORDER BY jitter_ms) AS median_jitter_ms,
            AVG(packet_loss_pct) AS packet_loss_pct,
            COUNT(*)::bigint AS sample_count
        FROM results
        WHERE received_at >= NOW() - INTERVAL '5 minutes'
        GROUP BY device_id
        ON CONFLICT (device_id, bucket_start)
        DO UPDATE SET sample_count = EXCLUDED.sample_count;
        "#,
    )
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/5m] computed with medians and p10/p90 percentiles");
    Ok(())
}

async fn compute_1hr_rollup(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO rollups_1hr
            (device_id, bucket_start, bucket_end,
             median_latency_ms, p10_latency_ms, p90_latency_ms,
             median_throughput_mbps, p10_throughput_mbps, p90_throughput_mbps,
             median_jitter_ms, packet_loss_pct, sample_count)
        SELECT
            device_id,
            date_trunc('hour', NOW() - INTERVAL '1 hour') AS bucket_start,
            date_trunc('hour', NOW())                     AS bucket_end,
            percentile_cont(0.50) WITHIN GROUP (ORDER BY median_latency_ms) AS median_latency_ms,
            MIN(p10_latency_ms) AS p10_latency_ms,
            MAX(p90_latency_ms) AS p90_latency_ms,
            percentile_cont(0.50) WITHIN GROUP (ORDER BY median_throughput_mbps) AS median_throughput_mbps,
            MIN(p10_throughput_mbps) AS p10_throughput_mbps,
            MAX(p90_throughput_mbps) AS p90_throughput_mbps,
            percentile_cont(0.50) WITHIN GROUP (ORDER BY median_jitter_ms) AS median_jitter_ms,
            AVG(packet_loss_pct) AS packet_loss_pct,
            SUM(sample_count)::bigint AS sample_count
        FROM rollups_5min
        WHERE bucket_start >= NOW() - INTERVAL '1 hour'
        GROUP BY device_id
        ON CONFLICT (device_id, bucket_start)
        DO UPDATE SET sample_count = EXCLUDED.sample_count;
        "#,
    )
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/1h] computed");
    Ok(())
}

async fn compute_1day_rollup(pool: &PgPool) -> anyhow::Result<()> {
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
            $1 AS bucket_date,
            percentile_cont(0.50) WITHIN GROUP (ORDER BY median_latency_ms) AS median_latency_ms,
            MIN(p10_latency_ms) AS p10_latency_ms,
            MAX(p90_latency_ms) AS p90_latency_ms,
            percentile_cont(0.50) WITHIN GROUP (ORDER BY median_throughput_mbps) AS median_throughput_mbps,
            MIN(p10_throughput_mbps) AS p10_throughput_mbps,
            MAX(p90_throughput_mbps) AS p90_throughput_mbps,
            percentile_cont(0.50) WITHIN GROUP (ORDER BY median_jitter_ms) AS median_jitter_ms,
            AVG(packet_loss_pct) AS packet_loss_pct,
            SUM(sample_count)::bigint AS sample_count
        FROM rollups_1hr
        WHERE bucket_start::date = $2
        GROUP BY device_id
        ON CONFLICT (device_id, bucket_date)
        DO UPDATE SET sample_count = EXCLUDED.sample_count;
        "#,
    )
    .bind(yesterday)
    .bind(yesterday)
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/1d] computed for {yesterday}");
    Ok(())
}
