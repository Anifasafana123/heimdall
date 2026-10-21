use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    app::AppState,
    auth,
    error::{AppError, AppResult},
    models::{
        Computer, IngestMetricsRequest, LoginRequest, Metric, RegisterComputerRequest,
        TokenResponse,
    },
};

// ---------- Auth ----------

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<TokenResponse>> {
    let user: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, password_hash FROM admin_users WHERE username = ?",
    )
    .bind(&payload.username)
    .fetch_optional(&state.pool)
    .await?;

    let (_, hash) = user.ok_or(AppError::Unauthorized)?;

    let valid = bcrypt::verify(&payload.password, &hash).map_err(|_| AppError::Unauthorized)?;
    if !valid {
        return Err(AppError::Unauthorized);
    }

    let token = auth::create_token(
        &payload.username,
        &state.config.jwt_secret,
        state.config.jwt_expiration_hours,
    )?;

    Ok(Json(TokenResponse {
        token,
        username: payload.username,
    }))
}

// ---------- Computers (admin) ----------

pub async fn list_computers(State(state): State<AppState>) -> AppResult<Json<Vec<Computer>>> {
    let computers = sqlx::query_as::<_, Computer>("SELECT * FROM computers ORDER BY registered_at DESC")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(computers))
}

pub async fn get_computer(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Computer>> {
    let computer = sqlx::query_as::<_, Computer>("SELECT * FROM computers WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(computer))
}

pub async fn register_computer(
    State(state): State<AppState>,
    Json(payload): Json<RegisterComputerRequest>,
) -> AppResult<Json<Computer>> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO computers (id, hostname, mac_address, ip_address, owner, is_authorized)
         VALUES (?, ?, ?, ?, ?, true)",
    )
    .bind(&id)
    .bind(&payload.hostname)
    .bind(&payload.mac_address)
    .bind(&payload.ip_address)
    .bind(&payload.owner)
    .execute(&state.pool)
    .await?;

    let computer = sqlx::query_as::<_, Computer>("SELECT * FROM computers WHERE id = ?")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(computer))
}

pub async fn set_authorization(
    State(state): State<AppState>,
    Path((id, authorized)): Path<(Uuid, bool)>,
) -> AppResult<Json<Computer>> {
    sqlx::query("UPDATE computers SET is_authorized = ? WHERE id = ?")
        .bind(authorized)
        .bind(id.to_string())
        .execute(&state.pool)
        .await?;

    let computer = sqlx::query_as::<_, Computer>("SELECT * FROM computers WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(computer))
}

// ---------- Metrics (agent ingest + admin read) ----------

/// Agent endpoint: ingest a batch of approved metrics.
pub async fn ingest_metrics(
    State(state): State<AppState>,
    Json(payload): Json<IngestMetricsRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Verify the computer exists and is authorized.
    let computer: Option<(bool,)> =
        sqlx::query_as("SELECT is_authorized FROM computers WHERE id = ?")
            .bind(payload.computer_id.to_string())
            .fetch_optional(&state.pool)
            .await?;
    let authorized = computer.ok_or(AppError::NotFound)?.0;
    if !authorized {
        return Err(AppError::Forbidden);
    }

    // Refresh last_seen_at to the most recent sample in this batch so the
    // dashboard status and heartbeat sweep stay accurate.
    if let Some(latest) = payload.metrics.iter().map(|m| m.recorded_at).max() {
        sqlx::query("UPDATE computers SET last_seen_at = ? WHERE id = ?")
            .bind(latest)
            .bind(payload.computer_id.to_string())
            .execute(&state.pool)
            .await?;
    }

    let mut stored = 0usize;
    for sample in &payload.metrics {
        if !crate::models::is_approved_metric(&sample.metric_kind) {
            tracing::warn!(kind = %sample.metric_kind, "rejected unapproved metric");
            continue;
        }
        sqlx::query(
            "INSERT INTO metrics (computer_id, metric_kind, value, recorded_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(payload.computer_id.to_string())
        .bind(&sample.metric_kind)
        .bind(sample.value)
        .bind(sample.recorded_at)
        .execute(&state.pool)
        .await?;
        stored += 1;
    }

    Ok(Json(serde_json::json!({ "stored": stored })))
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    pub metric_kind: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u64>,
}

/// Admin endpoint: read metrics for a computer (optionally filtered).
pub async fn get_metrics(
    State(state): State<AppState>,
    Path(computer_id): Path<Uuid>,
    Query(q): Query<MetricsQuery>,
) -> AppResult<Json<Vec<Metric>>> {
    let limit = q.limit.unwrap_or(500).min(5000);
    let computer_id = computer_id.to_string();
    let metrics = sqlx::query_as::<_, Metric>(
        "SELECT * FROM metrics
         WHERE computer_id = ?
           AND (? IS NULL OR metric_kind = ?)
           AND (? IS NULL OR recorded_at >= ?)
           AND (? IS NULL OR recorded_at <= ?)
         ORDER BY recorded_at DESC
         LIMIT ?",
    )
    .bind(&computer_id)
    .bind(&q.metric_kind)
    .bind(&q.metric_kind)
    .bind(q.from)
    .bind(q.from)
    .bind(q.to)
    .bind(q.to)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(metrics))
}

// ---------- Simple health ----------

pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
