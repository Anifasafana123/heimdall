//! Task 7 — Dead Computer Detector (FR14, FR12)
//!
//! Runs every 2 minutes. Finds computers that missed heartbeats and classifies
//! the gap as OUTAGE or OFFLINE (clean shutdown marker present).

use chrono::{DateTime, Utc};
use sqlx::MySqlPool;
use tracing::{info, warn};

const HEARTBEAT_INTERVAL_SECS: i64 = 60;
const MISS_THRESHOLD: i64 = 3;
const GAP_SECS: i64 = HEARTBEAT_INTERVAL_SECS * MISS_THRESHOLD; // 180 s

pub async fn run(pool: MySqlPool) {
    info!("[dead_computer_detector] started — gap threshold = {GAP_SECS}s");

    loop {
        if let Err(e) = scan(&pool).await {
            tracing::error!("[dead_computer_detector] error: {e}");
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(120)).await;
    }
}

async fn scan(pool: &MySqlPool) -> anyhow::Result<()> {
    // Devices whose heartbeat is stale and have no open connectivity event
    let stale = sqlx::query(
        r#"
        SELECT d.device_id, d.office_id, d.last_heartbeat_at
        FROM devices d
        WHERE d.last_heartbeat_at <= NOW() - INTERVAL 180 SECOND
          AND d.is_active = TRUE
          AND NOT EXISTS (
              SELECT 1 FROM connectivity_events ce
              WHERE ce.device_id = d.device_id
                AND ce.ended_at IS NULL
          )
        "#
    )
    .fetch_all(pool)
    .await?;

    for row in &stale {
        use sqlx::Row;
        let device_id: String = row.try_get("device_id")?;
        let office_id: String = row.try_get("office_id")?;
        let last_heartbeat_at: DateTime<Utc> = row.try_get("last_heartbeat_at")?;

        let event_type = classify(pool, &device_id, &office_id, last_heartbeat_at).await?;

        warn!(
            "[dead_computer_detector] device={} office={} last_seen={} → {}",
            device_id,
            office_id,
            last_heartbeat_at.format("%Y-%m-%d %H:%M:%S"),
            event_type.as_str()
        );

        let corroborated = event_type == EventType::Outage;

        sqlx::query(
            r#"
            INSERT INTO connectivity_events
                (device_id, office_id, event_type, started_at, ended_at, corroborated)
            VALUES (?, ?, ?, ?, NULL, ?)
            "#
        )
        .bind(&device_id)
        .bind(&office_id)
        .bind(event_type.as_str())
        .bind(last_heartbeat_at)
        .bind(corroborated)
        .execute(pool)
        .await?;
    }

    close_recovered_events(pool).await?;

    Ok(())
}

async fn classify(
    pool: &MySqlPool,
    device_id: &str,
    office_id: &str,
    last_heartbeat_at: DateTime<Utc>,
) -> anyhow::Result<EventType> {
    // Check for a clean shutdown marker near the last heartbeat
    let has_shutdown: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM agent_lifecycle_events
        WHERE device_id = ?
          AND event = 'SHUTDOWN'
          AND event_at >= ? - INTERVAL 5 MINUTE
        "#
    )
    .bind(device_id)
    .bind(last_heartbeat_at)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if has_shutdown > 0 {
        return Ok(EventType::Offline);
    }

    // Check if sibling computers in the same office also lost heartbeat
    let _siblings_also_lost: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM devices d
        WHERE d.office_id = ?
          AND d.device_id != ?
          AND d.last_heartbeat_at <= NOW() - INTERVAL 180 SECOND
          AND d.is_active = TRUE
        "#
    )
    .bind(office_id)
    .bind(device_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    Ok(EventType::Outage)
}

async fn close_recovered_events(pool: &MySqlPool) -> anyhow::Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE connectivity_events ce
        JOIN devices d ON ce.device_id = d.device_id
        SET ce.ended_at = d.last_heartbeat_at
        WHERE ce.ended_at IS NULL
          AND d.last_heartbeat_at > NOW() - INTERVAL 180 SECOND
        "#
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
