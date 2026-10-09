use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use tracing::{error, info};

use crate::WorkerState;
use crate::handlers::execute_request::{ExecuteRequest, ExecuteResponse};

pub async fn execute_handler(
    State(state): State<Arc<WorkerState>>,
    Json(req): Json<ExecuteRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(
        "Executing task: {} (type: {})",
        req.task_name, req.executor_type
    );

    let Some(executor_type) = req.executor_type.known() else {
        return Ok(Json(ExecuteResponse::Failed {
            error: format!("Unsupported executor type: {}", req.executor_type),
        }));
    };

    let result = czar_executors::dispatch::dispatch(
        executor_type,
        Some(state.working_dir.clone()),
        workflow_root(&state),
        req.task_id,
        &req.task_name,
        req.params.clone(),
    )
    .await;

    match result {
        Ok(output) => {
            let deferred = output.get("deferred").and_then(|v| v.as_array()).cloned();

            if deferred.is_some() {
                info!("Task {} returned deferrables", req.task_name);
            }

            Ok(Json(ExecuteResponse::Success { output, deferred }))
        }
        Err(e) => {
            error!("Task {} failed: {}", req.task_name, e);
            Ok(Json(ExecuteResponse::Failed { error: e }))
        }
    }
}

fn workflow_root(state: &WorkerState) -> Option<PathBuf> {
    let workflow_path = Path::new(&state.workflow_path);
    workflow_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .or_else(|| {
            if state.working_dir.is_empty() {
                None
            } else {
                Some(PathBuf::from(&state.working_dir))
            }
        })
}
