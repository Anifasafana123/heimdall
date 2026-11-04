//! Task 1 — Ingest Worker
//!
//! Polls the `pending_results` queue table every 5 seconds.
//! Validates incoming metric results and appends them to `results` (the main, append-only table).
//! Sequence number check: rejects any result whose seq <= the last accepted seq
//! for that device, preventing replay attacks (SR-C2).
//!
//! Archive integrity: past results are never updated or deleted (SR-C4).

use chrono::Utc;
use sqlx::PgPool;
use tracing::{error, info, warn};

pub async fn run(pool: PgPool) {
    info!("[ingest_worker] started (PostgreSQL append-only ingest)");

    loop {
        if let Err(e) = process_batch(&pool).await {
            error!("[ingest_worker] error: {e}");
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

async fn process_batch(pool: &PgPool) -> anyhow::Result<()> {
    let rows = sqlx::query(
        r#"
        SELECT id, device_id, seq, payload, received_at
        FROM pending_results
        WHERE processed_at IS NULL
        ORDER BY id ASC
        LIMIT 100
        "#,
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    info!("[ingest_worker] processing {} pending result(s)", rows.len());

    for row in rows {
        use sqlx::Row;
        let id: i64 = row.try_get("id")?;
        let device_id: String = row.try_get("device_id")?;
        let seq: i64 = row.try_get("seq")?;

        // Replay check: look up the last accepted seq for this device (SR-C2)
        let last_seq: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(seq) FROM results WHERE device_id = $1",
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
                mark_processed(pool, id).await?;
                continue;
            }
        }

        // Extract payload from the pending row (JSONB, already validated upstream)
        let payload: serde_json::Value = sqlx::query_scalar(
            "SELECT payload FROM pending_results WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap_or(serde_json::Value::Null);

        // Compute payload hash (SHA-256 hex) for tamper-evidence (SR-C1)
        use sha2::{Digest, Sha256};
        let payload_str = payload.to_string();
        let mut hasher = Sha256::new();
        hasher.update(payload_str.as_bytes());
        let payload_hash = hex::encode(hasher.finalize());

        // Insert into the append-only results table (no DELETE/UPDATE permitted — SR-C4)
        sqlx::query(
            r#"INSERT INTO results
               (device_id, seq, payload, payload_hash, received_at)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (device_id, seq) DO NOTHING"#,
        )
        .bind(&device_id)
        .bind(seq)
        .bind(&payload)
        .bind(&payload_hash)
        .bind(Utc::now())
        .execute(pool)
        .await?;

        mark_processed(pool, id).await?;

        info!("[ingest_worker] accepted — device={} seq={}", device_id, seq);
    }

    Ok(())
}

async fn mark_processed(pool: &PgPool, id: i64) -> anyhow::Result<()> {
    sqlx::query("UPDATE pending_results SET processed_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
