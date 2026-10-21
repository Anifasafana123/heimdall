//! Application error type.

use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Database(e) => {
                tracing::error!(error = %e, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "a database error occurred".to_string())
            }
            AppError::Internal(e) => {
                tracing::error!(error = %e, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "an internal error occurred".to_string())
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[allow(dead_code)]
pub type AppResult<T> = Result<T, AppError>;
