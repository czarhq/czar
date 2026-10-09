use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::api::ApiServer;
use czar_core::services::workflow_yaml::{
    ApplyWorkflowYamlRequest, ExistingWorkflowBehavior, WorkflowServiceError, apply_workflow_yaml,
};

#[derive(Default, Deserialize)]
pub struct CreateWorkflowRequest {
    pub yaml: String,
    pub project: Option<String>,
    pub region: Option<String>,
    pub root: Option<String>,
    pub replace: Option<bool>,
}

pub async fn create_workflow(
    State(api): State<ApiServer>,
    Json(payload): Json<CreateWorkflowRequest>,
) -> impl IntoResponse {
    let project = payload.project.unwrap_or_else(|| "local".to_string());
    let region = payload.region.unwrap_or_else(|| "local".to_string());
    let root = payload
        .root
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let root = root.canonicalize().unwrap_or(root);

    let replace = payload.replace.unwrap_or(true);
    let existing_workflow = if replace {
        ExistingWorkflowBehavior::Replace
    } else {
        ExistingWorkflowBehavior::Error
    };
    let workflow = match apply_workflow_yaml(
        &*api.db,
        ApplyWorkflowYamlRequest::new(&payload.yaml, &root, &project, &region)
            .with_existing_workflow(existing_workflow)
            .with_persist_schedule_on_create(true),
    )
    .await
    {
        Ok(result) => result.workflow,
        Err(WorkflowServiceError::WorkflowAlreadyExists { .. }) => {
            return (
                StatusCode::CONFLICT,
                "Workflow already exists; pass replace=true to update tasks.".to_string(),
            )
                .into_response();
        }
        Err(WorkflowServiceError::InvalidYaml(err)) => {
            return (StatusCode::BAD_REQUEST, err.to_string()).into_response();
        }
        Err(WorkflowServiceError::InvalidWorkflow(msg))
        | Err(WorkflowServiceError::Compilation(msg)) => {
            return (StatusCode::BAD_REQUEST, msg).into_response();
        }
        Err(WorkflowServiceError::Storage(err)) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
        }
    };

    #[derive(Default, Serialize)]
    struct Resp {
        name: String,
        id: String,
    }

    Json(Resp {
        name: workflow.name,
        id: workflow.id.to_string(),
    })
    .into_response()
}
