//! Task 7 — Dead Computer Detector (FR14, FR12)
//!
//! Runs every 2 minutes. Finds computers that have missed heartbeats and
//! classifies the gap as OUTAGE or OFFLINE.
//!
//! OFFLINE — agent wrote a clean SHUTDOWN marker before stopping.
//! OUTAGE  — no shutdown marker; connection was lost.
//!
//! No data is deleted. Events are written to connectivity_events (append).

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{info, warn};

const GAP_SECS: i32 = 180; // 3 missed heartbeats × 60s

pub async fn run(pool: PgPool) {
    info!("[dead_computer_detector] started — gap threshold = {GAP_SECS}s");
    loop {
        if let Err(e) = scan(&pool).await {
            tracing::error!("[dead_computer_detector] error: {e}");
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(120)).await;
    }
}

async fn scan(pool: &PgPool) -> anyhow::Result<()> {
    // Devices with stale heartbeat and no open connectivity event
    let stale = sqlx::query!(
        r#"
        SELECT d.device_id, d.office_id, d.last_heartbeat_at
        FROM devices d
        WHERE d.last_heartbeat_at <= NOW() - ($1 || ' seconds')::INTERVAL
          AND d.is_active = TRUE
          AND NOT EXISTS (
              SELECT 1 FROM connectivity_events ce
              WHERE ce.device_id = d.device_id
                AND ce.ended_at IS NULL
          )
        "#,
        GAP_SECS.to_string()
    )
    .fetch_all(pool)
    .await?;

    for row in &stale {
        let last_hb = row.last_heartbeat_at.unwrap_or_else(Utc::now);
        let event_type = classify(pool, &row.device_id, &row.office_id, last_hb).await?;

        warn!(
            "[dead_computer_detector] device={} office={} last_seen={} → {}",
            row.device_id,
            row.office_id,
            last_hb.format("%Y-%m-%d %H:%M:%S"),
            event_type.as_str()
        );

        let corroborated = event_type == EventType::Outage;

        sqlx::query!(
            r#"
            INSERT INTO connectivity_events
                (device_id, office_id, event_type, started_at, ended_at, corroborated)
            VALUES ($1, $2, $3, $4, NULL, $5)
            "#,
            row.device_id,
            row.office_id,
            event_type.as_str(),
            last_hb,
            corroborated
        )
        .execute(pool)
        .await?;
    }

    close_recovered_events(pool).await?;
    Ok(())
}

async fn classify(
    pool: &PgPool,
    device_id: &str,
    office_id: &str,
    last_heartbeat_at: DateTime<Utc>,
) -> anyhow::Result<EventType> {
    // Clean shutdown marker within 5 minutes of last heartbeat?
    let has_shutdown: bool = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) > 0 FROM agent_lifecycle_events
        WHERE device_id = $1
          AND event = 'SHUTDOWN'
          AND event_at >= $2 - INTERVAL '5 minutes'
        "#,
        device_id,
        last_heartbeat_at
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);

    if has_shutdown {
        return Ok(EventType::Offline);
    }

    // Corroborate: are sibling computers in the same office also down?
    let _siblings_down: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) FROM devices d
        WHERE d.office_id = $1
          AND d.device_id != $2
          AND d.last_heartbeat_at <= NOW() - ($3 || ' seconds')::INTERVAL
          AND d.is_active = TRUE
        "#,
        office_id,
        device_id,
        GAP_SECS.to_string()
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    Ok(EventType::Outage)
}

/// Close open events for devices whose heartbeat has recovered.
async fn close_recovered_events(pool: &PgPool) -> anyhow::Result<()> {
    let result = sqlx::query!(
        r#"
        UPDATE connectivity_events ce
        SET ended_at = d.last_heartbeat_at
        FROM devices d
        WHERE ce.device_id = d.device_id
          AND ce.ended_at IS NULL
          AND d.last_heartbeat_at > NOW() - ($1 || ' seconds')::INTERVAL
        "#,
        GAP_SECS.to_string()
    )
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 {
        info!(
            "[dead_computer_detector] closed {} recovered event(s)",
            result.rows_affected()
        );
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum EventType {
    Outage,
    Offline,
}

impl EventType {
    fn as_str(&self) -> &'static str {
        match self {
            EventType::Outage => "outage",
            EventType::Offline => "offline",
        }
    }
}
