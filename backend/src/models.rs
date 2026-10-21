use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A registered, authorized computer on the network.
///
/// `id` is stored as CHAR(36) in MySQL and mapped to `String` here.
/// The API layer validates it as a UUID at the boundary.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Computer {
    pub id: String,
    pub hostname: String,
    pub mac_address: String,
    pub ip_address: String,
    pub owner: String,
    pub is_authorized: bool,
    pub registered_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// A metric sample reported by a monitoring agent.
///
/// Only approved metrics are stored. `metric_kind` is constrained by the
/// agent contract (cpu_usage, memory_usage, disk_usage, network_rx, network_tx).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Metric {
    pub id: i64,
    pub computer_id: String,
    pub metric_kind: String,
    pub value: f64,
    pub recorded_at: DateTime<Utc>,
}

/// Admin user (dashboard login).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AdminUser {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

// --- Request payloads ---

#[derive(Debug, Deserialize)]
pub struct RegisterComputerRequest {
    pub hostname: String,
    pub mac_address: String,
    pub ip_address: String,
    pub owner: String,
}

#[derive(Debug, Deserialize)]
pub struct IngestMetricsRequest {
    pub computer_id: Uuid,
    pub metrics: Vec<MetricSample>,
}

#[derive(Debug, Deserialize)]
pub struct MetricSample {
    pub metric_kind: String,
    pub value: f64,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

// --- Response helpers ---

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token: String,
    pub username: String,
}

/// Allowed/approved metric kinds. Anything outside this set is rejected.
pub const APPROVED_METRICS: &[&str] =
    &["cpu_usage", "memory_usage", "disk_usage", "network_rx", "network_tx"];

pub fn is_approved_metric(kind: &str) -> bool {
    APPROVED_METRICS.iter().any(|m| *m == kind)
}
