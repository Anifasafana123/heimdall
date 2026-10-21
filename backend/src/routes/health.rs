//! GET /health — liveness and database reachability check.

use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use crate::{db, state::AppState};

pub async fn handler(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match db::health_check(&state.pool).await {
        Ok(()) => {
            let stats = db::pool_stats(&state.pool);
            (StatusCode::OK, Json(json!({
                "status":   "ok",
                "database": "ok",
                "pool":     stats,
            })))
        }
        Err(reason) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "error", "database": reason })),
        ),
    }
}
