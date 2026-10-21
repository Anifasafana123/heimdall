use axum::{
    extract::{Request, State},
    http::{
        header::{AUTHORIZATION, COOKIE},
        StatusCode,
    },
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // username
    pub exp: usize,
}

/// Create a signed JWT for an admin user.
pub fn create_token(
    username: &str,
    secret: &str,
    expiration_hours: i64,
) -> Result<String, AppError> {
    let exp = (Utc::now() + Duration::hours(expiration_hours)).timestamp() as usize;
    let claims = Claims {
        sub: username.into(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("jwt encode: {e}")))
}

/// Verify a JWT string and return its claims.
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| AppError::Unauthorized)?;
    Ok(data.claims)
}

/// Axum middleware: protects routes behind a valid admin JWT.
///
/// Expects `Authorization: Bearer <token>`. The secret is passed as
/// middleware state (the JWT secret string).
pub async fn require_auth(
    State(secret): State<String>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());

    match token {
        Some(t) => match verify_token(&t, &secret) {
            Ok(claims) => {
                req.extensions_mut().insert(claims.sub);
                Ok(next.run(req).await)
            }
            Err(_) => Err(StatusCode::UNAUTHORIZED),
        },
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Dashboard (HTML) auth middleware: reads the `token` cookie set by `/login`
/// and verifies the JWT. On failure, redirects the browser to `/login`.
///
/// This guards the HTMX dashboard views (page + partials). The `/login` route
/// itself stays public so there's no redirect loop.
pub async fn require_cookie_auth(
    State(secret): State<String>,
    mut req: Request,
    next: Next,
) -> Response {
    let cookie_header = req
        .headers()
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = cookie_header
        .split(';')
        .find_map(|c| c.trim().strip_prefix("token=").map(str::to_string));

    match token.and_then(|t| verify_token(&t, &secret).ok()) {
        Some(claims) => {
            // Make the admin username available to view handlers.
            req.extensions_mut().insert(claims.sub);
            next.run(req).await
        }
        None => Redirect::to("/login").into_response(),
    }
}
