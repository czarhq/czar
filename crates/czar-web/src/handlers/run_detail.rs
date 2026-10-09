use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

use crate::api::{ApiServer, fmt_time, is_not_found};
use crate::workflow_groups::{
    parse_task_groups_from_source_yaml, parse_workflow_definition_from_source_yaml,
};
use czar_core::models::json_inner;

#[derive(Default, Serialize)]
pub struct RunDetail {
    pub run: RunInfo,
    pub tasks: Vec<TaskInfo>,
    pub workflow: Option<WorkflowInfo>,
}

#[derive(Default, Serialize)]
pub struct RunInfo {
    pub id: String,
    pub workflow: String,
    pub status: String,
    pub task_selection: Option<serde_json::Value>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Default, Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub executor: String,
    pub depends_on: Vec<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub dispatched_at: Option<String>,
    pub output: Option<serde_json::Value>,
    pub logs: Option<String>,
    pub error: Option<String>,
    pub progress: Option<serde_json::Value>,
}

#[derive(Default, Serialize, Clone)]
pub struct WorkflowInfo {
    pub name: String,
    pub tasks: Vec<WorkflowTaskInfo>,
    pub task_groups: BTreeMap<String, Vec<String>>,
    pub flows: Vec<String>,
    pub schedule: Option<String>,
    pub schedule_enabled: bool,
    pub next_scheduled_at: Option<String>,
    pub source_yaml: Option<String>,
}

#[derive(Default, Serialize, Clone)]
pub struct WorkflowTaskLastStateInfo {
    pub status: String,
    pub run_id: String,
    pub task_id: String,
    pub finished_at: Option<String>,
    pub success_at: Option<String>,
    pub failure_at: Option<String>,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Default, Serialize, Clone)]
pub struct WorkflowTaskInfo {
    pub name: String,
    pub depends_on: Vec<String>,
    pub executor: String,
    pub params: Option<serde_json::Value>,
    pub last_state: Option<WorkflowTaskLastStateInfo>,
}

#[derive(Deserialize, Default)]
pub struct RunDetailQuery {
    /// When `slim=1`, omit logs, output, and error from task responses.
    /// Use this for polling and grid views where those fields are not displayed.
    #[serde(default)]
    slim: bool,
}

pub async fn run_detail(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
    Query(query): Query<RunDetailQuery>,
) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let run = match api.db.get_run(run_id).await {
        Ok(run) => run,
        Err(err) => {
            if is_not_found(&err) {
                return StatusCode::NOT_FOUND.into_response();
            }
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let workflow = api.db.get_workflow_by_id(run.workflow_id).await.ok();
    let workflow_name = workflow
        .as_ref()
        .map(|wf| wf.name.clone())
        .unwrap_or_else(|| run.workflow_id.to_string());

    let mut task_rows = match api.db.list_tasks(run_id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    task_rows.sort_by_key(|a| a.task_index);

    let tasks: Vec<TaskInfo> = task_rows
        .into_iter()
        .map(|task| {
            let status = task.status_str().to_string();
            TaskInfo {
                id: task.id.to_string(),
                name: task.task_name,
                status,
                executor: task.executor_type,
                depends_on: task.depends_on,
                started_at: task.started_at.map(fmt_time),
                finished_at: task.finished_at.map(fmt_time),
                dispatched_at: task.dispatched_at.map(fmt_time),
                output: if query.slim {
                    None
                } else {
                    task.output.as_ref().map(|out| json_inner(out).clone())
                },
                logs: if query.slim { None } else { task.logs },
                error: if query.slim { None } else { task.error },
                progress: task.progress.as_ref().map(|p| json_inner(p).clone()),
            }
        })
        .collect();

    let workflow_info = if let Some(workflow) = workflow.as_ref() {
        let mut workflow_tasks = api
            .db
            .list_workflow_tasks(workflow.id)
            .await
            .unwrap_or_default();
        workflow_tasks.sort_by_key(|a| a.task_index);
        let task_states = api
            .db
            .list_workflow_task_states(workflow.id)
            .await
            .unwrap_or_default();
        let task_state_by_name: HashMap<String, WorkflowTaskLastStateInfo> = task_states
            .into_iter()
            .map(|state| {
                (
                    state.task_name,
                    WorkflowTaskLastStateInfo {
                        status: state.last_status.as_str().to_string(),
                        run_id: state.last_run_id.to_string(),
                        task_id: state.last_task_id.to_string(),
                        finished_at: state.last_finished_at.map(fmt_time),
                        success_at: state.last_success_at.map(fmt_time),
                        failure_at: state.last_failure_at.map(fmt_time),
                        output: state.last_output.map(|v| json_inner(&v).clone()),
                        error: state.last_error,
                    },
                )
            })
            .collect();
        let source_yaml = if let Some(snapshot_id) = workflow.current_snapshot_id {
            api.db
                .get_snapshot(snapshot_id)
                .await
                .ok()
                .and_then(|s| s.source_yaml)
        } else {
            None
        };
        let task_groups = source_yaml
            .as_deref()
            .and_then(|yaml| parse_task_groups_from_source_yaml(yaml).ok())
            .unwrap_or_default();
        let flows = source_yaml
            .as_deref()
            .and_then(|yaml| parse_workflow_definition_from_source_yaml(yaml).ok())
            .map(|definition| definition.flow_names())
            .unwrap_or_default();

        Some(WorkflowInfo {
            name: workflow.name.clone(),
            tasks: workflow_tasks
                .into_iter()
                .map(|task| {
                    let task_name = task.task_name;
                    WorkflowTaskInfo {
                        name: task_name.clone(),
                        depends_on: task.depends_on,
                        executor: task.executor_type,
                        params: task.params.map(|p| p.0),
                        last_state: task_state_by_name.get(&task_name).cloned(),
                    }
                })
                .collect(),
            task_groups,
            flows,
            schedule: workflow.schedule.clone(),
            schedule_enabled: workflow.schedule_enabled,
            next_scheduled_at: workflow.next_scheduled_at.map(fmt_time),
            source_yaml,
        })
    } else {
        None
    };

    Json(RunDetail {
        run: RunInfo {
            id: run.id.to_string(),
            workflow: workflow_name,
            status: run.status_str().to_string(),
            task_selection: run.task_selection.map(|v| json_inner(&v).clone()),
            created_at: fmt_time(run.created_at),
            started_at: run.started_at.map(fmt_time),
            finished_at: run.finished_at.map(fmt_time),
            error: run.error,
        },
        tasks,
        workflow: workflow_info,
    })
    .into_response()
}
