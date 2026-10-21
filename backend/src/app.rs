use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use sqlx::MySqlPool;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{auth, handlers};

/// Shared application state available to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub config: crate::config::Config,
}

/// Build the Axum router — JSON REST API only.
/// Dashboard / UI is issue #4 (Farwes).
pub fn app(state: AppState) -> Router {
    // Public routes — no auth required
    let public = Router::new()
        .route("/health", get(handlers::health))
        .route("/auth/login", post(handlers::login))
        .route("/api/agents/ingest", post(handlers::ingest_metrics));

    // Admin routes — require JWT bearer token
    let protected = Router::new()
        .route(
            "/api/computers",
            get(handlers::list_computers).post(handlers::register_computer),
        )
        .route("/api/computers/:id", get(handlers::get_computer))
        .route(
            "/api/computers/:id/authorize/:authorized",
            post(handlers::set_authorization),
        )
        .route("/api/computers/:id/metrics", get(handlers::get_metrics))
        .route_layer(middleware::from_fn_with_state(
            state.config.jwt_secret.clone(),
            auth::require_auth,
        ));

    Router::new()
        .merge(public)
        .merge(protected)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
