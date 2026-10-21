use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use sqlx::MySqlPool;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};

use crate::{
    auth, config::Config, handlers,
    views,
};

/// Shared application state available to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub config: Config,
}

/// Build the full Axum router.
pub fn app(state: AppState) -> Router {
    // Public API routes
    let public = Router::new()
        .route("/health", get(handlers::health))
        .route("/auth/login", post(handlers::login))
        // Agent ingest endpoint
        .route("/api/agents/ingest", post(handlers::ingest_metrics));

    // Protected API routes (require admin JWT)
    let protected = Router::new()
        .route("/api/computers", get(handlers::list_computers).post(handlers::register_computer))
        .route("/api/computers/:id", get(handlers::get_computer))
        .route("/api/computers/:id/authorize/:authorized", post(handlers::set_authorization))
        .route("/api/computers/:id/metrics", get(handlers::get_metrics))
        .route_layer(middleware::from_fn_with_state(
            state.config.jwt_secret.clone(),
            auth::require_auth,
        ));

    // HTMX dashboard views (HTML fragments + full pages)
    // `/login` stays public so auth redirects don't loop.
    let dashboard_public = Router::new()
        .route("/login", get(views::login_page).post(views::login_submit));

    // Everything else on the dashboard requires the `token` cookie.
    let dashboard_protected = Router::new()
        .route("/", get(views::dashboard_page))
        .route("/logout", get(views::logout))
        .route("/partials/computers", get(views::computers_partial))
        .route("/partials/metrics/:id", get(views::metrics_partial))
        .route_layer(middleware::from_fn_with_state(
            state.config.jwt_secret.clone(),
            auth::require_cookie_auth,
        ));

    Router::new()
        .merge(public)
        .merge(protected)
        .merge(dashboard_public)
        .merge(dashboard_protected)
        // Static assets (CSS, JS) for the HTMX dashboard
        .nest_service("/static", ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
