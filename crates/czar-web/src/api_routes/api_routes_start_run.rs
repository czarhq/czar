use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use czar_core::models::{RunTaskSelection, TaskSelectionMode};
use czar_core::task_execution::resolve_task_selection;

use crate::api::{ApiServer, StateUpdate};
use crate::api_routes::load_visible_workflow;
use crate::workflow_groups::{
    build_snapshot_tasks_for_flow, expand_group_tasks, parse_workflow_definition_from_source_yaml,
    resolve_flow_workflow_from_source_yaml,
};

#[derive(Default, Deserialize)]
pub(crate) struct StartRunRequest {
    workflow: String,
    #[serde(default)]
    flow: Option<String>,
    #[serde(default)]
    selection: Option<StartRunSelection>,
}

#[derive(Default, Deserialize)]
pub(crate) struct StartRunSelection {
    #[serde(default)]
    task_names: Vec<String>,
    #[serde(default)]
    group_names: Vec<String>,
    #[serde(default)]
    mode: Option<String>,
}

pub(crate) async fn start_run(
    State(api): State<ApiServer>,
    Json(payload): Json<StartRunRequest>,
) -> impl IntoResponse {
    let StartRunRequest {
        workflow: workflow_name,
        flow: requested_flow,
        selection: selection_payload,
    } = payload;

    let workflow = match load_visible_workflow(&api, &workflow_name).await {
        Ok(wf) => wf,
        Err(err) => {
            if crate::api::is_not_found(&err) {
                return StatusCode::NOT_FOUND.into_response();
            }
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let source_yaml = if let Some(snapshot_id) = workflow.current_snapshot_id {
        match api.db.get_snapshot(snapshot_id).await {
            Ok(snapshot) => snapshot.source_yaml,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    } else {
        None
    };
    let Some(source_yaml) = source_yaml else {
        return (
            StatusCode::BAD_REQUEST,
            "workflow source YAML is missing; re-apply the workflow definition".to_string(),
        )
            .into_response();
    };

    let definition = match parse_workflow_definition_from_source_yaml(&source_yaml) {
        Ok(definition) => definition,
        Err(err) => return (StatusCode::BAD_REQUEST, err).into_response(),
    };
    let selected_flow = match definition.selected_flow_name(requested_flow.as_deref()) {
        Ok(flow_name) => flow_name,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    let flow_workflow =
        match resolve_flow_workflow_from_source_yaml(&source_yaml, requested_flow.as_deref()) {
            Ok(flow_workflow) => flow_workflow,
            Err(err) => return (StatusCode::BAD_REQUEST, err).into_response(),
        };
    let flow_name_for_metadata = definition
        .flows
        .contains_key(&selected_flow)
        .then(|| selected_flow.clone());

    let base_workflow_tasks = match api.db.list_workflow_tasks(workflow.id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let snapshot_tasks = match build_snapshot_tasks_for_flow(&base_workflow_tasks, &flow_workflow) {
        Ok(tasks) => tasks,
        Err(err) => return (StatusCode::BAD_REQUEST, err).into_response(),
    };
    let tasks_json = match serde_json::to_value(&snapshot_tasks) {
        Ok(tasks_json) => tasks_json,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let content_hash = compute_content_hash(&tasks_json);
    let snapshot = match api
        .db
        .create_or_get_snapshot(
            workflow.id,
            &content_hash,
            tasks_json,
            Some(source_yaml.clone()),
        )
        .await
    {
        Ok(snapshot) => snapshot,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let task_graph: Vec<(String, Vec<String>)> = flow_workflow
        .tasks
        .iter()
        .map(|(name, task)| (name.clone(), task.depends_on.clone()))
        .collect();
    let mut run_selection: Option<RunTaskSelection> = None;
    if let Some(selection) = selection_payload {
        let mut requested_tasks: Vec<String> = selection
            .task_names
            .iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();

        let requested_groups: Vec<String> = selection
            .group_names
            .iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();
        if !requested_groups.is_empty() {
            let group_tasks = match expand_group_tasks(&definition.task_groups, &requested_groups) {
                Ok(tasks) => tasks,
                Err(err) => return (StatusCode::BAD_REQUEST, err).into_response(),
            };
            requested_tasks.extend(group_tasks);
        }

        requested_tasks.sort();
        requested_tasks.dedup();
        if requested_tasks.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                "selection requires at least one task or group".to_string(),
            )
                .into_response();
        }

        let mode = match selection.mode.as_deref() {
            Some(raw) => match TaskSelectionMode::parse(raw) {
                Some(mode) => mode,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        format!(
                            "invalid selection.mode '{}'; expected exact|upstream|downstream|full",
                            raw
                        ),
                    )
                        .into_response();
                }
            },
            None => TaskSelectionMode::Full,
        };
        let resolved_tasks = match resolve_task_selection(&task_graph, &requested_tasks, mode) {
            Ok(tasks) => tasks,
            Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
        };
        run_selection = Some(RunTaskSelection {
            mode,
            flow_name: flow_name_for_metadata.clone(),
            requested_tasks,
            requested_groups,
            resolved_tasks,
        });
    } else if requested_flow.is_some() {
        let resolved_tasks: Vec<String> = task_graph.iter().map(|(name, _)| name.clone()).collect();
        run_selection = Some(RunTaskSelection {
            mode: TaskSelectionMode::Exact,
            flow_name: flow_name_for_metadata.clone(),
            requested_tasks: resolved_tasks.clone(),
            requested_groups: Vec::new(),
            resolved_tasks,
        });
    }
    let task_selection_json = match run_selection {
        Some(selection) => match serde_json::to_value(selection) {
            Ok(value) => Some(value),
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        None => None,
    };

    match api
        .db
        .create_run_with_snapshot_and_selection(
            workflow.id,
            Some(snapshot.id),
            "ui",
            task_selection_json,
        )
        .await
    {
        Ok(run) => {
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: run.id.to_string(),
                status: run.status().as_str().to_string(),
            });
            #[derive(Default, Serialize)]
            struct Resp {
                run_id: String,
            }
            Json(Resp {
                run_id: run.id.to_string(),
            })
            .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn compute_content_hash(tasks_json: &serde_json::Value) -> String {
    let canonical = serde_json::to_string(tasks_json).unwrap_or_else(|_| "null".to_string());
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}
