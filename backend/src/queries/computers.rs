//! Queries for the `computers` table.

use sqlx::PgPool;
use crate::error::AppResult;

#[derive(Debug)]
#[allow(dead_code)]
pub struct ComputerRecord {
    pub id:                  i64,
    pub office_id:           i64,
    pub last_accepted_seq:   i64,
    pub revoked_at:          Option<chrono::DateTime<chrono::Utc>>,
    pub pending_approval_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Look up a computer by device_id (= certificate serial, PROTOCOL.md §3).
pub async fn find_by_device_id(
    pool:      &PgPool,
    device_id: &str,
) -> AppResult<Option<ComputerRecord>> {
    let row = sqlx::query_as!(
        ComputerRecord,
        r#"
        SELECT id, office_id, last_accepted_seq, revoked_at, pending_approval_at
        FROM   computers
        WHERE  device_id  = $1
          AND  deleted_at IS NULL
        LIMIT 1
        "#,
        device_id,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Count active (not revoked, not deleted) computers for an office.
/// Used to enforce the per-office limit (FR5).
pub async fn count_active_for_office(
    pool:      &PgPool,
    office_id: i64,
) -> AppResult<i64> {
    let row = sqlx::query!(
        r#"
        SELECT COUNT(*) AS count
        FROM   computers
        WHERE  office_id  = $1
          AND  revoked_at IS NULL
          AND  deleted_at IS NULL
        "#,
        office_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(row.count.unwrap_or(0))
}
