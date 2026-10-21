//! Task 6 — Rollup Aggregator (FR22)
//!
//! Computes 5-minute, 1-hour, and 1-day aggregate stats per device.
//! All published figures are MEDIAN with P10/P90 bands — never averages.
//! An average hides an outage; a median with a band does not (FR22).
//!
//! Schedule:
//!   5-min  → every 5 minutes
//!   1-hr   → every hour
//!   1-day  → daily at 00:05 UTC

use chrono::{Duration, Utc};
use sqlx::PgPool;
use tracing::info;

pub async fn run(pool: PgPool) {
    info!("[rollup_aggregator] started");

    let p5m = pool.clone();
    let p1h = pool.clone();
    let p1d = pool.clone();

    let t1 = tokio::spawn(async move {
        loop {
            if let Err(e) = compute_5min_rollup(&p5m).await {
                tracing::error!("[rollup_aggregator/5m] {e}");
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
        }
    });

    let t2 = tokio::spawn(async move {
        loop {
            if let Err(e) = compute_1hr_rollup(&p1h).await {
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

            if let Err(e) = compute_1day_rollup(&p1d).await {
                tracing::error!("[rollup_aggregator/1d] {e}");
            }
        }
    });

    let _ = tokio::join!(t1, t2, t3);
}

/// 5-minute rollup using PERCENTILE_CONT for median, p10, p90 (FR22).
async fn compute_5min_rollup(pool: &PgPool) -> anyhow::Result<()> {
    // Use date_trunc to get a clean 5-min bucket
    sqlx::query!(
        r#"
        INSERT INTO rollups_5min
            (device_id, bucket_start, bucket_end,
             median_latency_ms,      p10_latency_ms,      p90_latency_ms,
             median_throughput_mbps, p10_throughput_mbps, p90_throughput_mbps,
             median_jitter_ms,       packet_loss_pct,     sample_count)
        SELECT
            device_id,
            date_trunc('minute', NOW()) - INTERVAL '5 minutes' AS bucket_start,
            date_trunc('minute', NOW())                         AS bucket_end,
            PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY latency_ms),
            PERCENTILE_CONT(0.10) WITHIN GROUP (ORDER BY latency_ms),
            PERCENTILE_CONT(0.90) WITHIN GROUP (ORDER BY latency_ms),
            PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY throughput_mbps),
            PERCENTILE_CONT(0.10) WITHIN GROUP (ORDER BY throughput_mbps),
            PERCENTILE_CONT(0.90) WITHIN GROUP (ORDER BY throughput_mbps),
            PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY jitter_ms),
            AVG(packet_loss_pct),
            COUNT(*)
        FROM results
        WHERE received_at >= NOW() - INTERVAL '5 minutes'
          AND id NOT IN (SELECT result_id FROM quarantine WHERE reviewed = FALSE)
        GROUP BY device_id
        ON CONFLICT (device_id, bucket_start) DO NOTHING
        "#
    )
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/5m] computed");
    Ok(())
}

/// 1-hour rollup: re-aggregate from 5-min rollup rows using percentile.
async fn compute_1hr_rollup(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO rollups_1hr
            (device_id, bucket_start, bucket_end,
             median_latency_ms,      p10_latency_ms,      p90_latency_ms,
             median_throughput_mbps, p10_throughput_mbps, p90_throughput_mbps,
             median_jitter_ms,       packet_loss_pct,     sample_count)
        SELECT
            device_id,
            date_trunc('hour', NOW()) - INTERVAL '1 hour' AS bucket_start,
            date_trunc('hour', NOW())                      AS bucket_end,
            PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY median_latency_ms),
            MIN(p10_latency_ms),
            MAX(p90_latency_ms),
            PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY median_throughput_mbps),
            MIN(p10_throughput_mbps),
            MAX(p90_throughput_mbps),
            PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY median_jitter_ms),
            AVG(packet_loss_pct),
            SUM(sample_count)
        FROM rollups_5min
        WHERE bucket_start >= NOW() - INTERVAL '1 hour'
        GROUP BY device_id
        ON CONFLICT (device_id, bucket_start) DO NOTHING
        "#
    )
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/1h] computed");
    Ok(())
}

/// 1-day rollup: aggregate from 1-hr rollup rows for the previous calendar day.
async fn compute_1day_rollup(pool: &PgPool) -> anyhow::Result<()> {
    let yesterday = (Utc::now() - Duration::days(1)).date_naive();

    sqlx::query!(
        r#"
        INSERT INTO rollups_1day
            (device_id, bucket_date,
             median_latency_ms,      p10_latency_ms,      p90_latency_ms,
             median_throughput_mbps, p10_throughput_mbps, p90_throughput_mbps,
             median_jitter_ms,       packet_loss_pct,     sample_count)
        SELECT
            device_id,
            $1 AS bucket_date,
            PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY median_latency_ms),
            MIN(p10_latency_ms),
            MAX(p90_latency_ms),
            PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY median_throughput_mbps),
            MIN(p10_throughput_mbps),
            MAX(p90_throughput_mbps),
            PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY median_jitter_ms),
            AVG(packet_loss_pct),
            SUM(sample_count)
        FROM rollups_1hr
        WHERE bucket_start::date = $1
        GROUP BY device_id
        ON CONFLICT (device_id, bucket_date) DO NOTHING
        "#,
        yesterday
    )
    .execute(pool)
    .await?;

    info!("[rollup_aggregator/1d] computed for {yesterday}");
    Ok(())
}
