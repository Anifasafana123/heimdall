use axum::{
    extract::{Extension, Form, Path, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use minijinja::{context, Environment};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    app::AppState,
    auth,
    error::{AppError, AppResult},
    models::Computer,
};

/// Shared MiniJinja environment with the dashboard templates loaded.
/// Returns an error instead of panicking so a template syntax mistake
/// surfaces as a clean HTTP 500 rather than crashing the worker thread.
fn env() -> AppResult<Environment<'static>> {
    let mut env = Environment::new();
    env.add_template("layout", include_str!("../templates/layout.html"))
        .map_err(|e| AppError::Internal(format!("template layout: {e}")))?;
    env.add_template("login", include_str!("../templates/login.html"))
        .map_err(|e| AppError::Internal(format!("template login: {e}")))?;
    env.add_template("dashboard", include_str!("../templates/dashboard.html"))
        .map_err(|e| AppError::Internal(format!("template dashboard: {e}")))?;
    env.add_template("computers_partial", include_str!("../templates/computers_partial.html"))
        .map_err(|e| AppError::Internal(format!("template computers_partial: {e}")))?;
    env.add_template("metrics_partial", include_str!("../templates/metrics_partial.html"))
        .map_err(|e| AppError::Internal(format!("template metrics_partial: {e}")))?;
    Ok(env)
}

fn render(template: &str, ctx: minijinja::value::Value) -> AppResult<Html<String>> {
    let env = env()?;
    let rendered = env
        .get_template(template)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .render(ctx)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Html(rendered))
}

// ---------- Pages ----------

pub async fn login_page() -> AppResult<Html<String>> {
    render("login", context! {})
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn login_submit(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> AppResult<Response> {
    let user: Option<(i64, String)> =
        sqlx::query_as("SELECT id, password_hash FROM admin_users WHERE username = ?")
            .bind(&form.username)
            .fetch_optional(&state.pool)
            .await?;

    let (_, hash) = user.ok_or(AppError::Unauthorized)?;
    let valid = bcrypt::verify(&form.password, &hash).map_err(|_| AppError::Unauthorized)?;
    if !valid {
        return Err(AppError::Unauthorized);
    }

    let token = auth::create_token(
        &form.username,
        &state.config.jwt_secret,
        state.config.jwt_expiration_hours,
    )?;

    // Set token in a cookie and redirect to the dashboard.
    Ok((
        [("set-cookie", format!("token={token}; Path=/; HttpOnly; SameSite=Strict"))],
        Redirect::to("/"),
    )
        .into_response())
}

pub async fn dashboard_page(Extension(username): Extension<String>) -> AppResult<Html<String>> {
    render("dashboard", context! { username })
}

/// Clear the session cookie and send the browser back to the login page.
pub async fn logout() -> impl IntoResponse {
    (
        [(
            "set-cookie",
            "token=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        )],
        Redirect::to("/login"),
    )
}

// ---------- HTMX Partials ----------

pub async fn computers_partial(State(state): State<AppState>) -> AppResult<Html<String>> {
    let computers = sqlx::query_as::<_, Computer>(
        "SELECT * FROM computers ORDER BY registered_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    render("computers_partial", context! { computers })
}

pub async fn metrics_partial(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    let id = id.to_string();
    let metrics = sqlx::query_as::<_, crate::models::Metric>(
        "SELECT * FROM metrics WHERE computer_id = ? ORDER BY recorded_at DESC LIMIT 50",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;

    let computer = sqlx::query_as::<_, Computer>("SELECT * FROM computers WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?;

    render("metrics_partial", context! { computer, metrics })
}
