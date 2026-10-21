//! Background Processing — Issue #5 (Safana)
//!
//! All long-running Tokio async tasks for HEIMDALL are defined here.
//! Each task runs in its own `tokio::spawn` loop and is independent.
//!
//! Tasks:
//!   1. ingest_worker        — saves incoming metric results to the DB
//!   2. anomaly_detector     — flags impossible / suspicious values (SR-C3)
//!   3. speedtest_lease_mgr  — ensures only one speedtest per office at a time (FR30)
//!   4. daily_ceiling_watcher — enforces per-office daily bandwidth limit (FR32)
//!   5. merkle_builder       — builds daily Merkle tree for archive integrity (SR-C4)
//!   6. rollup_aggregator    — computes 5-min / 1-hr / 1-day aggregate stats
//!   7. dead_computer_detector — detects missing heartbeats; outage vs powered-off (FR14)

pub mod anomaly_detector;
pub mod daily_ceiling_watcher;
pub mod dead_computer_detector;
pub mod ingest_worker;
pub mod merkle_builder;
pub mod rollup_aggregator;
pub mod speedtest_lease_mgr;

use sqlx::PgPool;
use tracing::info;

/// Start every background task. Each task runs forever in its own Tokio green thread.
pub async fn start_all(pool: PgPool) {
    info!("Starting all background tasks...");

    tokio::spawn(ingest_worker::run(pool.clone()));
    tokio::spawn(anomaly_detector::run(pool.clone()));
    tokio::spawn(speedtest_lease_mgr::run(pool.clone()));
    tokio::spawn(daily_ceiling_watcher::run(pool.clone()));
    tokio::spawn(merkle_builder::run(pool.clone()));
    tokio::spawn(rollup_aggregator::run(pool.clone()));
    tokio::spawn(dead_computer_detector::run(pool.clone()));

    info!("All background tasks spawned.");
}
