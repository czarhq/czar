use crate::handlers::execute_handler;
use crate::handlers::execute_request::{ExecuteRequest, RequestedExecutorType};
use axum::Json;
use axum::body::to_bytes;
use axum::extract::State;
use axum::response::IntoResponse;
use czar_core::executor::StatusUpdate;
use czar_core::models::ExecutorType;
use std::fs;
use std::sync::Once;
use uuid::Uuid;

fn init_tracing() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::INFO)
            .try_init();
    });
}

async fn run_handler(req: ExecuteRequest) -> serde_json::Value {
    run_handler_with_state(
        crate::WorkerState {
            workflow_path: "workflow.yaml".to_string(),
            working_dir: ".".to_string(),
        },
        req,
    )
    .await
}

async fn run_handler_with_state(
    state: crate::WorkerState,
    req: ExecuteRequest,
) -> serde_json::Value {
    init_tracing();
    let state = std::sync::Arc::new(state);
    let response = execute_handler(State(state), Json(req))
        .await
        .expect("handler should return Ok response")
        .into_response();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&body).expect("response should be valid json")
}

mod process;
mod python;
mod wait_updates;
mod wasm;
