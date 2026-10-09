use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use czar_core::workflow::ExecutorKind;
use czar_core::workflow_source::compile_workflow_from_path;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info};

use crate::WorkerState;

#[derive(Debug, Default, Serialize)]
pub struct CompileResponse {
    pub workflow: serde_json::Value,
    pub tasks: Vec<TaskDefinition>,
    /// Raw source YAML so the orchestrator can register the workflow definition.
    pub source_yaml: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct TaskDefinition {
    pub name: String,
    pub executor: ExecutorKind,
    pub depends_on: Vec<String>,
}

pub async fn compile_handler(
    State(state): State<Arc<WorkerState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!("Compiling workflow from: {}", state.workflow_path);

    let source_yaml =
        czar_core::workflow_source::load_workflow_source(Path::new(&state.workflow_path)).ok();

    let (workflow, compiled) = compile_workflow_from_path(Path::new(&state.workflow_path))
        .map_err(|e| {
            error!("Failed to compile workflow: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string())
        })?;

    let tasks = compiled
        .tasks
        .iter()
        .map(|task| TaskDefinition {
            name: task.name.clone(),
            executor: task.executor.clone(),
            depends_on: task
                .depends_on
                .iter()
                .filter_map(|&idx| compiled.tasks.get(idx).map(|t| t.name.clone()))
                .collect(),
        })
        .collect();

    info!(
        "Successfully compiled workflow with {} tasks",
        compiled.tasks.len()
    );

    let workflow_json = serde_json::to_value(&workflow).map_err(|e| {
        error!("Failed to serialize workflow: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(CompileResponse {
        workflow: workflow_json,
        tasks,
        source_yaml,
    }))
}

#[cfg(test)]
mod tests;
