//! Task 3 — Speedtest Lease Manager (FR30)
//!
//! Guarantees only ONE computer per office runs a full speedtest at a time.
//! Runs every 10 seconds:
//!   1. Expires leases held longer than MAX_LEASE_SECS.
//!   2. Grants the oldest pending lease for each office with no active lease.

use sqlx::MySqlPool;
use tracing::{info, warn};

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

async fn expire_stale_leases(pool: &MySqlPool) -> anyhow::Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE speedtest_leases
        SET status = 'expired', expired_at = NOW()
        WHERE status = 'active'
          AND granted_at <= NOW() - INTERVAL 60 SECOND
        "#
    )
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 {
        warn!(
            "[speedtest_lease_mgr] expired {} stale lease(s)",
            result.rows_affected()
        );
    }

    Ok(())
}

async fn grant_pending_leases(pool: &MySqlPool) -> anyhow::Result<()> {
    // Find offices that have a pending lease but no active lease
    let offices: Vec<String> = sqlx::query_scalar(
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
        let lease_id: Option<u64> = sqlx::query_scalar(
            r#"
            SELECT sl.id
            FROM speedtest_leases sl
            JOIN devices d ON sl.device_id = d.device_id
            WHERE sl.status = 'pending'
              AND d.office_id = ?
            ORDER BY sl.requested_at ASC
            LIMIT 1
            "#
        )
        .bind(&office_id)
        .fetch_one(pool)
        .await?;

        if let Some(id) = lease_id {
            sqlx::query(
                "UPDATE speedtest_leases SET status = 'active', granted_at = NOW() WHERE id = ?"
            )
            .bind(id)
            .execute(pool)
            .await?;

            info!("[speedtest_lease_mgr] granted lease id={id} for office={office_id}");
        }
    }

    Ok(())
}
