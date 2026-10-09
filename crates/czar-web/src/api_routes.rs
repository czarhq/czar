use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use anyhow::anyhow;
use czar_core::database::RunListQuery;
use czar_core::models::{RunStatus, json_inner};
use czar_core::workflow_registry::workflow_is_active_for_definitions_root;

use crate::handlers::{WorkflowInfo, WorkflowTaskInfo, WorkflowTaskLastStateInfo};
use crate::workflow_groups::{
    parse_task_groups_from_source_yaml, parse_workflow_definition_from_source_yaml,
};

use crate::api::{ApiServer, fmt_time, is_not_found};
mod api_routes_control;
mod api_routes_start_run;
pub(crate) use api_routes_control::{
    cancel_run, pause_run, pause_task, resume_run, resume_task, update_workflow_schedule,
};
pub(crate) use api_routes_start_run::start_run;

#[derive(Default, Deserialize)]
pub(crate) struct RunQueryParams {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    search: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    workflow: Option<String>,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    until: Option<String>,
}

#[derive(Default, Serialize)]
struct RunListItem {
    id: String,
    workflow: String,
    status: String,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

#[derive(Default, Serialize)]
struct PaginatedRuns {
    items: Vec<RunListItem>,
    total: usize,
    limit: usize,
    offset: usize,
    has_more: bool,
}

pub(crate) async fn list_runs(
    State(api): State<ApiServer>,
    Query(params): Query<RunQueryParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let search = params
        .search
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let statuses: Vec<RunStatus> = params
        .status
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| RunStatus::parse(s.trim()))
        .collect();
    let workflow_names: Vec<String> = params
        .workflow
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect();
    fn parse_date_start(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
    }
    fn parse_date_end(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(23, 59, 59))
            .map(|dt| dt.and_utc())
    }
    let since = params.since.as_deref().and_then(parse_date_start);
    let until = params.until.as_deref().and_then(parse_date_end);
    let query = RunListQuery {
        limit,
        offset,
        search,
        statuses,
        workflow_names,
        since,
        until,
    };
    let page = match api.db.list_runs_page(&query).await {
        Ok(page) => page,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let items: Vec<RunListItem> = page
        .items
        .into_iter()
        .map(|entry| RunListItem {
            id: entry.run.id.to_string(),
            workflow: entry
                .workflow_name
                .unwrap_or_else(|| entry.run.workflow_id.to_string()),
            status: entry.run.status().as_str().to_string(),
            created_at: fmt_time(entry.run.created_at),
            started_at: entry.run.started_at.map(fmt_time),
            finished_at: entry.run.finished_at.map(fmt_time),
        })
        .collect();

    let has_more = offset + items.len() < page.total;

    Json(PaginatedRuns {
        items,
        total: page.total,
        limit,
        offset,
        has_more,
    })
    .into_response()
}

#[derive(Default, Deserialize)]
pub(crate) struct WorkflowQueryParams {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    search: Option<String>,
}

#[derive(Default, Serialize)]
struct WorkflowListItem {
    id: String,
    name: String,
    executor: String,
    project: String,
    region: String,
    schedule: Option<String>,
    schedule_enabled: bool,
    next_scheduled_at: Option<String>,
}

#[derive(Default, Serialize)]
struct PaginatedWorkflows {
    items: Vec<WorkflowListItem>,
    total: usize,
    limit: usize,
    offset: usize,
    has_more: bool,
}

pub(crate) async fn list_workflows(
    State(api): State<ApiServer>,
    Query(params): Query<WorkflowQueryParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100).min(200);
    let offset = params.offset.unwrap_or(0);
    let search = params
        .search
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    let mut workflows = match api.db.list_workflows().await {
        Ok(workflows) => workflows,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    workflows.retain(|workflow| {
        workflow_is_active_for_definitions_root(workflow, api.workflow_definitions_root())
    });
    workflows.sort_by(|a, b| a.name.cmp(&b.name));
    if let Some(search) = search.as_deref() {
        workflows.retain(|workflow| workflow.name.to_lowercase().contains(search));
    }
    let total = workflows.len();
    let page_items = workflows
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();

    let items: Vec<WorkflowListItem> = page_items
        .into_iter()
        .map(|wf| WorkflowListItem {
            id: wf.id.to_string(),
            name: wf.name,
            executor: wf.executor_type,
            project: wf.project,
            region: wf.region,
            schedule: wf.schedule,
            schedule_enabled: wf.schedule_enabled,
            next_scheduled_at: wf.next_scheduled_at.map(fmt_time),
        })
        .collect();

    let has_more = offset + items.len() < total;

    Json(PaginatedWorkflows {
        items,
        total,
        limit,
        offset,
        has_more,
    })
    .into_response()
}

pub(crate) async fn workflow_detail(
    Path(name): Path<String>,
    State(api): State<ApiServer>,
) -> impl IntoResponse {
    let workflow = match load_visible_workflow(&api, &name).await {
        Ok(wf) => wf,
        Err(err) => {
            if is_not_found(&err) {
                return StatusCode::NOT_FOUND.into_response();
            }
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let mut tasks = match api.db.list_workflow_tasks(workflow.id).await {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    tasks.sort_by_key(|a| a.task_index);
    let task_states = match api.db.list_workflow_task_states(workflow.id).await {
        Ok(states) => states,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let task_state_by_name = task_states
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
        .collect::<HashMap<_, _>>();
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
    let tasks = tasks
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
        .collect();
    Json(WorkflowInfo {
        name: workflow.name,
        tasks,
        task_groups,
        flows,
        schedule: workflow.schedule,
        schedule_enabled: workflow.schedule_enabled,
        next_scheduled_at: workflow.next_scheduled_at.map(fmt_time),
        source_yaml,
    })
    .into_response()
}

#[derive(Default, Serialize)]
struct WorkflowTaskStateListItem {
    task_name: String,
    last_status: Option<String>,
    last_run_id: Option<String>,
    last_task_id: Option<String>,
    run_api_path: Option<String>,
    finished_at: Option<String>,
    success_at: Option<String>,
    failure_at: Option<String>,
    output: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Default, Serialize)]
struct WorkflowTaskStateListResponse {
    workflow: String,
    items: Vec<WorkflowTaskStateListItem>,
}

pub(crate) async fn workflow_task_states(
    Path(name): Path<String>,
    State(api): State<ApiServer>,
) -> impl IntoResponse {
    let workflow = match load_visible_workflow(&api, &name).await {
        Ok(wf) => wf,
        Err(err) => {
            if is_not_found(&err) {
                return StatusCode::NOT_FOUND.into_response();
            }
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut workflow_tasks = match api.db.list_workflow_tasks(workflow.id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    workflow_tasks.sort_by_key(|a| a.task_index);

    let task_states = match api.db.list_workflow_task_states(workflow.id).await {
        Ok(states) => states,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let state_by_name: HashMap<String, WorkflowTaskLastStateInfo> = task_states
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

    let items = workflow_tasks
        .into_iter()
        .map(|task| {
            let task_name = task.task_name;
            let state = state_by_name.get(&task_name);
            let run_id = state.map(|s| s.run_id.clone());
            let task_id = state.map(|s| s.task_id.clone());
            WorkflowTaskStateListItem {
                task_name,
                last_status: state.map(|s| s.status.clone()),
                last_run_id: run_id.clone(),
                last_task_id: task_id.clone(),
                run_api_path: run_id.map(|id| format!("/api/runs/{id}")),
                finished_at: state.and_then(|s| s.finished_at.clone()),
                success_at: state.and_then(|s| s.success_at.clone()),
                failure_at: state.and_then(|s| s.failure_at.clone()),
                output: state.and_then(|s| s.output.clone()),
                error: state.and_then(|s| s.error.clone()),
            }
        })
        .collect();

    Json(WorkflowTaskStateListResponse {
        workflow: workflow.name,
        items,
    })
    .into_response()
}

pub(crate) async fn load_visible_workflow(
    api: &ApiServer,
    name: &str,
) -> anyhow::Result<czar_core::models::Workflow> {
    let workflow = api.db.get_workflow(name).await?;
    if workflow_is_active_for_definitions_root(&workflow, api.workflow_definitions_root()) {
        Ok(workflow)
    } else {
        Err(anyhow!("workflow not found: {name}"))
    }
}
