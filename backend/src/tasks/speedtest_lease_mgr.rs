//! Task 3 — Speedtest Lease Manager (FR30)
//!
//! Guarantees that only ONE computer per office runs a full speedtest at a time.
//!
//! How it works:
//!   - Agents POST /lease/speedtest to request a lease before testing.
//!     The API handler inserts a row into `speedtest_leases` with status='pending'.
//!   - This task runs every 10 seconds and:
//!       1. Expires leases that have been active longer than MAX_LEASE_SECS.
//!       2. Grants the oldest pending lease for each office that has no active lease.
//!
//! The agent receives the lease token in the API response and presents it
//! when uploading speedtest results. Without a valid lease, speedtest results
//! are rejected by the ingest worker.

use sqlx::MySqlPool;
use tracing::{info, warn};

/// Maximum time a lease may be held before it is expired (seconds).
/// An 8-second test + 30-second buffer = 38 s; we use 60 s to be safe.
const MAX_LEASE_SECS: u64 = 60;

pub async fn run(pool: MySqlPool) {
    info!("[speedtest_lease_mgr] started — max lease = {MAX_LEASE_SECS}s");

    loop {
        if let Err(e) = tick(&pool).await {
            tracing::error!("[speedtest_lease_mgr] error: {e}");
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
}

async fn tick(pool: &MySqlPool) -> anyhow::Result<()> {
    expire_stale_leases(pool).await?;
    grant_pending_leases(pool).await?;
    Ok(())
}

/// Expire any active lease that started more than MAX_LEASE_SECS ago.
async fn expire_stale_leases(pool: &MySqlPool) -> anyhow::Result<()> {
    let expired = sqlx::query!(
        r#"
        UPDATE speedtest_leases
        SET status = 'expired', expired_at = NOW()
        WHERE status = 'active'
          AND granted_at <= NOW() - INTERVAL ? SECOND
        "#,
        MAX_LEASE_SECS
    )
    .execute(pool)
    .await?;

    if expired.rows_affected() > 0 {
        warn!(
            "[speedtest_lease_mgr] expired {} stale lease(s)",
            expired.rows_affected()
        );
    }

    Ok(())
}

/// For each office that currently has NO active lease, grant its oldest pending request.
async fn grant_pending_leases(pool: &MySqlPool) -> anyhow::Result<()> {
    // Find all offices that have a pending lease and no active lease
    let offices: Vec<String> = sqlx::query_scalar!(
        r#"
        SELECT DISTINCT d.office_id
        FROM speedtest_leases sl
        JOIN devices d ON sl.device_id = d.device_id
        WHERE sl.status = 'pending'
          AND d.office_id NOT IN (
              SELECT d2.office_id
              FROM speedtest_leases sl2
              JOIN devices d2 ON sl2.device_id = d2.device_id
              WHERE sl2.status = 'active'
          )
        "#
    )
    .fetch_all(pool)
    .await?;

    for office_id in offices {
        // Pick the oldest pending lease for this office
        let lease_id: Option<u64> = sqlx::query_scalar!(
            r#"
            SELECT sl.id
            FROM speedtest_leases sl
            JOIN devices d ON sl.device_id = d.device_id
            WHERE sl.status = 'pending'
              AND d.office_id = ?
            ORDER BY sl.requested_at ASC
            LIMIT 1
            "#,
            office_id
        )
        .fetch_one(pool)
        .await?;

        if let Some(id) = lease_id {
            sqlx::query!(
                "UPDATE speedtest_leases SET status = 'active', granted_at = NOW() WHERE id = ?",
                id
            )
            .execute(pool)
            .await?;

            info!(
                "[speedtest_lease_mgr] granted lease id={id} for office={office_id}"
            );
        }
    }

    Ok(())
}
