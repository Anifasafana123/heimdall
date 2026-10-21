//! Queries for the `metrics` table.
//!
//! Envelope field names match PROTOCOL.md §3 exactly.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use crate::error::AppResult;

// ── Input ─────────────────────────────────────────────────────────────────────

/// All fields needed to insert one signed envelope into `metrics`.
/// Field names match PROTOCOL.md §3.
#[derive(Debug)]
#[allow(dead_code)]
pub struct NewMetric {
    pub computer_id: i64,
    pub office_id:   i64,

    // Signing envelope fields (PROTOCOL.md §3)
    pub device_id:    String,
    pub seq:          i64,
    pub nonce:        String,   // base64url, 22 chars
    pub ts_agent:     i64,      // Unix seconds
    pub payload_type: String,
    pub payload_hash: String,   // base64url, 43 chars
    pub prev_hash:    String,   // base64url, 43 chars
    pub sig:          String,   // base64url, 86 chars

    // Measurement fields — all optional
    pub latency_min_ms:     Option<i32>,
    pub latency_max_ms:     Option<i32>,
    pub latency_mean_ms:    Option<i32>,
    pub latency_median_ms:  Option<i32>,
    pub latency_p95_ms:     Option<i32>,
    pub jitter_mean_ms:     Option<i32>,
    pub jitter_max_ms:      Option<i32>,
    pub packets_sent:       Option<i32>,
    pub packets_received:   Option<i32>,
    pub loss_percent:       Option<f64>,
    pub download_bps:       Option<i64>,
    pub upload_bps:         Option<i64>,
    pub throughput_streams: Option<i16>,
    pub interface_type:     Option<String>,
    pub link_speed_bps:     Option<i64>,
    pub mtu:                Option<i16>,
    pub dns_resolver_class: Option<String>,
    pub asn:                Option<i32>,
    pub probe_id:           Option<String>,
    pub uptime_secs:        Option<i64>,
}

/// Filter for time-range history queries.
#[derive(Debug)]
#[allow(dead_code)]
pub struct TimeRangeFilter {
    pub computer_id:  i64,
    pub from:         DateTime<Utc>,
    pub to:           DateTime<Utc>,
    pub payload_type: Option<String>,
    pub limit:        i64,
}

/// Filter for office-level history queries.
#[derive(Debug)]
#[allow(dead_code)]
pub struct OfficeHistoryFilter {
    pub office_id:    i64,
    pub from:         DateTime<Utc>,
    pub to:           DateTime<Utc>,
    pub payload_type: Option<String>,
    pub limit:        i64,
}

// ── Output ────────────────────────────────────────────────────────────────────

/// A metric row returned to the dashboard.
#[derive(Debug, serde::Serialize)]
pub struct MetricRow {
    pub id:                i64,
    pub computer_id:       i64,
    pub office_id:         i64,
    pub payload_type:      String,
    pub recorded_at:       DateTime<Utc>,
    pub latency_min_ms:    Option<i32>,
    pub latency_max_ms:    Option<i32>,
    pub latency_mean_ms:   Option<i32>,
    pub latency_median_ms: Option<i32>,
    pub latency_p95_ms:    Option<i32>,
    pub jitter_mean_ms:    Option<i32>,
    pub jitter_max_ms:     Option<i32>,
    pub loss_percent:      Option<f64>,
    pub download_bps:      Option<i64>,
    pub upload_bps:        Option<i64>,
    pub interface_type:    Option<String>,
    pub asn:               Option<i32>,
    pub quality:           String,
}

/// Percentile summary for FR22 — medians with p10/p90 band, never a maximum.
#[derive(Debug, serde::Serialize)]
pub struct OfficePercentiles {
    pub office_id:           i64,
    // Latency — percentile_cont on INTEGER returns FLOAT8
    pub latency_p10_ms:      Option<f64>,
    pub latency_median_ms:   Option<f64>,
    pub latency_p90_ms:      Option<f64>,
    // Download — percentile_cont on BIGINT returns FLOAT8
    pub download_p10_bps:    Option<f64>,
    pub download_median_bps: Option<f64>,
    pub download_p90_bps:    Option<f64>,
    // Upload
    pub upload_p10_bps:      Option<f64>,
    pub upload_median_bps:   Option<f64>,
    pub upload_p90_bps:      Option<f64>,
    // Loss — percentile_cont on NUMERIC returns NUMERIC, cast to f64
    pub loss_p10_pct:        Option<f64>,
    pub loss_median_pct:     Option<f64>,
    pub loss_p90_pct:        Option<f64>,
}

/// Last heartbeat time per computer in an office (public map live pin).
#[derive(Debug, serde::Serialize)]
pub struct ComputerLastSeen {
    pub computer_id:       i64,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
}

// ── Queries ───────────────────────────────────────────────────────────────────

/// Insert one signed metric envelope.
///
/// The UNIQUE constraint on (computer_id, seq) blocks replays at the DB
/// level — a duplicate seq returns a sqlx unique-violation error (SR-C2).
pub async fn insert_metric(pool: &PgPool, m: &NewMetric) -> AppResult<i64> {
    let row = sqlx::query!(
        r#"
        INSERT INTO metrics (
            computer_id, office_id,
            device_id, seq, nonce, ts_agent, payload_type,
            payload_hash, prev_hash, sig,
            latency_min_ms, latency_max_ms, latency_mean_ms,
            latency_median_ms, latency_p95_ms,
            jitter_mean_ms, jitter_max_ms,
            packets_sent, packets_received, loss_percent,
            download_bps, upload_bps, throughput_streams,
            interface_type, link_speed_bps, mtu,
            dns_resolver_class, asn, probe_id, uptime_secs,
            quality
        ) VALUES (
            $1,  $2,
            $3,  $4,  $5,  $6,  $7,
            $8,  $9,  $10,
            $11, $12, $13,
            $14, $15,
            $16, $17,
            $18, $19, $20,
            $21, $22, $23,
            $24, $25, $26,
            $27, $28, $29, $30,
            'ok'
        )
        RETURNING id
        "#,
        m.computer_id, m.office_id,
        m.device_id, m.seq, m.nonce, m.ts_agent, m.payload_type,
        m.payload_hash, m.prev_hash, m.sig,
        m.latency_min_ms, m.latency_max_ms, m.latency_mean_ms,
        m.latency_median_ms, m.latency_p95_ms,
        m.jitter_mean_ms, m.jitter_max_ms,
        m.packets_sent, m.packets_received, m.loss_percent,
        m.download_bps, m.upload_bps, m.throughput_streams,
        m.interface_type, m.link_speed_bps, m.mtu,
        m.dns_resolver_class, m.asn, m.probe_id, m.uptime_secs,
    )
    .fetch_one(pool)
    .await?;

    Ok(row.id)
}

/// Fetch the single most-recent quality='ok' row for a computer + payload_type.
pub async fn fetch_latest(
    pool:         &PgPool,
    computer_id:  i64,
    payload_type: &str,
) -> AppResult<Option<MetricRow>> {
    let row = sqlx::query_as!(
        MetricRow,
        r#"
        SELECT
            id, computer_id, office_id, payload_type,
            recorded_at,
            latency_min_ms, latency_max_ms, latency_mean_ms,
            latency_median_ms, latency_p95_ms,
            jitter_mean_ms, jitter_max_ms,
            loss_percent,
            download_bps, upload_bps,
            interface_type, asn,
            quality
        FROM metrics
        WHERE computer_id  = $1
          AND payload_type = $2
          AND quality      = 'ok'
        ORDER BY recorded_at DESC
        LIMIT 1
        "#,
        computer_id,
        payload_type,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Fetch metric rows for a computer within a time range, oldest-first.
pub async fn fetch_history(
    pool:   &PgPool,
    filter: &TimeRangeFilter,
) -> AppResult<Vec<MetricRow>> {
    let rows = sqlx::query_as!(
        MetricRow,
        r#"
        SELECT
            id, computer_id, office_id, payload_type,
            recorded_at,
            latency_min_ms, latency_max_ms, latency_mean_ms,
            latency_median_ms, latency_p95_ms,
            jitter_mean_ms, jitter_max_ms,
            loss_percent,
            download_bps, upload_bps,
            interface_type, asn,
            quality
        FROM metrics
        WHERE computer_id  = $1
          AND quality      = 'ok'
          AND recorded_at  BETWEEN $2 AND $3
          AND ($4::TEXT IS NULL OR payload_type = $4)
        ORDER BY recorded_at ASC
        LIMIT $5
        "#,
        filter.computer_id,
        filter.from,
        filter.to,
        filter.payload_type,
        filter.limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Fetch metric rows for a whole office within a time range.
pub async fn fetch_office_history(
    pool:   &PgPool,
    filter: &OfficeHistoryFilter,
) -> AppResult<Vec<MetricRow>> {
    let rows = sqlx::query_as!(
        MetricRow,
        r#"
        SELECT
            id, computer_id, office_id, payload_type,
            recorded_at,
            latency_min_ms, latency_max_ms, latency_mean_ms,
            latency_median_ms, latency_p95_ms,
            jitter_mean_ms, jitter_max_ms,
            loss_percent,
            download_bps, upload_bps,
            interface_type, asn,
            quality
        FROM metrics
        WHERE office_id   = $1
          AND quality     = 'ok'
          AND recorded_at BETWEEN $2 AND $3
          AND ($4::TEXT IS NULL OR payload_type = $4)
        ORDER BY recorded_at ASC, computer_id ASC
        LIMIT $5
        "#,
        filter.office_id,
        filter.from,
        filter.to,
        filter.payload_type,
        filter.limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Fetch p10 / median / p90 for an office over a time window.
///
/// FR22: published figures are always medians with a p10–p90 band.
/// Never a maximum, never a best moment.
/// Uses PostgreSQL percentile_cont() — not available in MySQL.
pub async fn fetch_office_percentiles(
    pool:      &PgPool,
    office_id: i64,
    from:      DateTime<Utc>,
    to:        DateTime<Utc>,
) -> AppResult<OfficePercentiles> {
    let row = sqlx::query!(
        r#"
        SELECT
            -- Latency (INTEGER → percentile_cont returns FLOAT8)
            percentile_cont(0.10) WITHIN GROUP (ORDER BY latency_mean_ms) AS "latency_p10_ms: f64",
            percentile_cont(0.50) WITHIN GROUP (ORDER BY latency_mean_ms) AS "latency_median_ms: f64",
            percentile_cont(0.90) WITHIN GROUP (ORDER BY latency_mean_ms) AS "latency_p90_ms: f64",
            -- Download (BIGINT → FLOAT8)
            percentile_cont(0.10) WITHIN GROUP (ORDER BY download_bps)    AS "download_p10_bps: f64",
            percentile_cont(0.50) WITHIN GROUP (ORDER BY download_bps)    AS "download_median_bps: f64",
            percentile_cont(0.90) WITHIN GROUP (ORDER BY download_bps)    AS "download_p90_bps: f64",
            -- Upload (BIGINT → FLOAT8)
            percentile_cont(0.10) WITHIN GROUP (ORDER BY upload_bps)      AS "upload_p10_bps: f64",
            percentile_cont(0.50) WITHIN GROUP (ORDER BY upload_bps)      AS "upload_median_bps: f64",
            percentile_cont(0.90) WITHIN GROUP (ORDER BY upload_bps)      AS "upload_p90_bps: f64",
            -- Loss (DOUBLE PRECISION)
            percentile_cont(0.10) WITHIN GROUP (ORDER BY loss_percent) AS "loss_p10_pct: f64",
            percentile_cont(0.50) WITHIN GROUP (ORDER BY loss_percent) AS "loss_median_pct: f64",
            percentile_cont(0.90) WITHIN GROUP (ORDER BY loss_percent) AS "loss_p90_pct: f64"
        FROM metrics
        WHERE office_id    = $1
          AND quality      = 'ok'
          AND payload_type = 'measurement'
          AND recorded_at  BETWEEN $2 AND $3
        "#,
        office_id,
        from,
        to,
    )
    .fetch_one(pool)
    .await?;

    Ok(OfficePercentiles {
        office_id,
        latency_p10_ms:      row.latency_p10_ms,
        latency_median_ms:   row.latency_median_ms,
        latency_p90_ms:      row.latency_p90_ms,
        download_p10_bps:    row.download_p10_bps,
        download_median_bps: row.download_median_bps,
        download_p90_bps:    row.download_p90_bps,
        upload_p10_bps:      row.upload_p10_bps,
        upload_median_bps:   row.upload_median_bps,
        upload_p90_bps:      row.upload_p90_bps,
        loss_p10_pct:        row.loss_p10_pct,
        loss_median_pct:     row.loss_median_pct,
        loss_p90_pct:        row.loss_p90_pct,
    })
}

/// Last heartbeat per computer in an office — public map live pin.
pub async fn fetch_office_last_seen(
    pool:      &PgPool,
    office_id: i64,
) -> AppResult<Vec<ComputerLastSeen>> {
    let rows = sqlx::query_as!(
        ComputerLastSeen,
        r#"
        SELECT
            computer_id,
            MAX(recorded_at) AS "last_heartbeat_at: DateTime<Utc>"
        FROM metrics
        WHERE office_id    = $1
          AND payload_type = 'heartbeat'
          AND quality      = 'ok'
        GROUP BY computer_id
        "#,
        office_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
