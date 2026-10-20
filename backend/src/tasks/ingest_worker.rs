//! Task 1 — Ingest Worker
//!
//! Polls the `pending_results` queue table every 5 seconds.
//! Moves validated rows into `results` (the main, append-only table).
//! Sequence number check: rejects any result whose seq <= the last accepted seq
//! for that device, preventing replays (SR-C2).

use chrono::Utc;
use sqlx::MySqlPool;
use tracing::{error, info, warn};

/// A raw row waiting in the ingest queue.
#[derive(Debug)]
struct PendingResult {
    id: u64,
    device_id: String,
    seq: i64,
    payload: serde_json::Value,
    received_at: chrono::DateTime<Utc>,
}

pub async fn run(pool: MySqlPool) {
    info!("[ingest_worker] started");

    loop {
        if let Err(e) = process_batch(&pool).await {
            error!("[ingest_worker] error: {e}");
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

async fn process_batch(pool: &MySqlPool) -> anyhow::Result<()> {
    let rows = sqlx::query(
        r#"
        SELECT id, device_id, seq, payload, received_at
        FROM pending_results
        ORDER BY id ASC
        LIMIT 100
        "#
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    info!("[ingest_worker] processing {} pending result(s)", rows.len());

    for row in rows {
        use sqlx::Row;
        let id: u64 = row.try_get::<u64, _>("id")?;
        let device_id: String = row.try_get("device_id")?;
        let seq: i64 = row.try_get("seq")?;

        // Replay check: look up the last accepted seq for this device
        let last_seq: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(seq) FROM results WHERE device_id = ?"
        )
        .bind(&device_id)
        .fetch_one(pool)
        .await?;

        if let Some(last) = last_seq {
            if seq <= last {
                warn!(
                    "[ingest_worker] replay detected — device={} seq={} last_accepted={}; dropping",
                    device_id, seq, last
                );
                delete_pending(pool, id).await?;
                continue;
            }
        }

        // Insert into the append-only results table
        sqlx::query(
            r#"INSERT INTO results (device_id, seq, payload, received_at) VALUES (?, ?, ?, ?)"#
        )
        .bind(&device_id)
        .bind(seq)
        .bind(serde_json::Value::Null) // payload stored as JSON
        .bind(chrono::Utc::now())
        .execute(pool)
        .await?;

        delete_pending(pool, id).await?;

        info!("[ingest_worker] accepted — device={} seq={}", device_id, seq);
    }

    Ok(())
}

async fn delete_pending(pool: &MySqlPool, id: u64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM pending_results WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
