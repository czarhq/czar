use axum::{Json, response::IntoResponse};

pub async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "czar-worker"
    }))
}
