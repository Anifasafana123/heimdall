//! Task 1 — Ingest Worker
//!
//! Polls the `pending_results` queue table every 5 seconds.
//! Moves validated rows into `results` (append-only — no DELETE ever).
//! Replay protection: rejects any result whose seq <= last accepted seq (SR-C2).

use sqlx::PgPool;
use tracing::{error, info, warn};

pub async fn run(pool: PgPool) {
    info!("[ingest_worker] started");
    loop {
        if let Err(e) = process_batch(&pool).await {
            error!("[ingest_worker] error: {e}");
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

async fn process_batch(pool: &PgPool) -> anyhow::Result<()> {
    // Fetch up to 100 pending rows — oldest first
    let rows = sqlx::query!(
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
        // Replay check: reject if seq <= last accepted seq for this device
        let last_seq: Option<i64> = sqlx::query_scalar!(
            "SELECT MAX(seq) FROM results WHERE device_id = $1",
            row.device_id
        )
        .fetch_one(pool)
        .await?;

        if let Some(last) = last_seq {
            if row.seq <= last {
                warn!(
                    "[ingest_worker] replay — device={} seq={} last={}; dropping",
                    row.device_id, row.seq, last
                );
                // Mark processed so we don't re-examine it
                sqlx::query!(
                    "UPDATE pending_results SET processed = TRUE WHERE id = $1",
                    row.id
                )
                .execute(pool)
                .await?;
                continue;
            }
        }

        // Insert into append-only results table
        sqlx::query!(
            r#"
            INSERT INTO results (device_id, seq, payload, received_at)
            VALUES ($1, $2, $3, $4)
            "#,
            row.device_id,
            row.seq,
            row.payload,
            row.received_at
        )
        .execute(pool)
        .await?;

        // Mark pending row as processed (no DELETE — audit trail)
        sqlx::query!(
            "UPDATE pending_results SET processed = TRUE WHERE id = $1",
            row.id
        )
        .execute(pool)
        .await?;

        info!("[ingest_worker] accepted — device={} seq={}", row.device_id, row.seq);
    }

    Ok(())
}
