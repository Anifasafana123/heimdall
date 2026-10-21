//! Queries for the `offices` table.

use sqlx::PgPool;
use crate::error::AppResult;

#[derive(Debug)]
#[allow(dead_code)]
pub struct OfficeRecord {
    pub id:            i64,
    pub name:          String,
    pub max_computers: i16,
    pub is_active:     bool,
}

/// Fetch an active office by its primary key.
pub async fn find_by_id(pool: &PgPool, id: i64) -> AppResult<Option<OfficeRecord>> {
    let row = sqlx::query_as!(
        OfficeRecord,
        r#"
        SELECT id, name, max_computers, is_active
        FROM   offices
        WHERE  id         = $1
          AND  is_active  = TRUE
          AND  deleted_at IS NULL
        LIMIT 1
        "#,
        id,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}
