use axum::{Router, middleware, routing::post};
use std::sync::Arc;

use crate::WorkerState;
use crate::auth::require_bearer_token;
use crate::handlers::{compile_handler, execute_handler, health_handler};

/// `auth_token` protects every route except `/health`, which readiness probes call.
pub fn build_app(state: WorkerState, auth_token: Option<String>) -> Router {
    let protected = Router::new()
        .route("/compile", post(compile_handler))
        .route("/execute", post(execute_handler));
    let protected = match auth_token {
        Some(token) => protected.route_layer(middleware::from_fn_with_state(
            Arc::<str>::from(token),
            require_bearer_token,
        )),
        None => protected,
    };
    Router::new()
        .route("/health", post(health_handler))
        .merge(protected)
        .with_state(Arc::new(state))
}
