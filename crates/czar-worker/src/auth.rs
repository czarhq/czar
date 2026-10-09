//! Optional shared bearer token that the orchestrator must send
//! (`czar-worker --auth-token`, `token:` in the orchestrator's workers config).

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

pub async fn require_bearer_token(
    State(token): State<Arc<str>>,
    request: Request,
    next: Next,
) -> Response {
    if !bearer_matches(request.headers(), &token) {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            "missing or invalid bearer token",
        )
            .into_response();
    }
    next.run(request).await
}

// Helpers

fn bearer_matches(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|given| given.as_bytes().ct_eq(token.as_bytes()).into())
}
