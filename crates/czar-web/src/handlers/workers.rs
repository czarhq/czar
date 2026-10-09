use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use czar_core::services::worker_sync::sync_worker;
use czar_core::worker_registry::WorkerConfig;

use crate::api::ApiServer;

#[derive(Deserialize)]
pub struct RegisterWorkerRequest {
    pub url: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub poll_interval_secs: Option<u64>,
}

#[derive(Serialize)]
pub struct RegisterWorkerResponse {
    pub worker_url: String,
    pub workflow_name: String,
}

pub async fn register_worker(
    State(api): State<ApiServer>,
    Json(req): Json<RegisterWorkerRequest>,
) -> impl IntoResponse {
    let config = WorkerConfig {
        url: req.url,
        name: req.name,
        poll_interval_secs: req.poll_interval_secs,
        token: None,
        token_env: None,
    };

    match sync_worker(&*api.db, &config).await {
        Ok(result) => Json(RegisterWorkerResponse {
            worker_url: result.worker_url,
            workflow_name: result.workflow_name,
        })
        .into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}
