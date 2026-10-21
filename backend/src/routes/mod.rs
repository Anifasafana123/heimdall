//! HTTP route definitions.

use axum::{routing::get, Router};
use crate::state::AppState;

pub mod health;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::handler))
        .with_state(state)
}
