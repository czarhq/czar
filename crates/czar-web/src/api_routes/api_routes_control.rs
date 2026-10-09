use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use uuid::Uuid;

use czar_core::models::{RunStatus, TaskStatus, json_inner};

use crate::api::{ApiServer, StateUpdate, is_not_found};
use crate::api_routes::load_visible_workflow;

pub(crate) async fn cancel_run(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let run = match api.db.get_run(run_id).await {
        Ok(run) => run,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match api.db.cancel_run(run_id).await {
        Ok(_) => {
            if let Ok(tasks) = api.db.list_tasks(run_id).await {
                for task in tasks {
                    if matches!(task.status(), TaskStatus::Cancelled) {
                        let _ = api
                            .db
                            .upsert_workflow_task_state(
                                run.workflow_id,
                                &task.task_name,
                                TaskStatus::Cancelled,
                                run_id,
                                task.id,
                                task.output.as_ref().map(|v| json_inner(v).clone()),
                                task.error.as_deref(),
                                task.finished_at,
                            )
                            .await;
                    }
                }
            }
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: id.clone(),
                status: RunStatus::Cancelled.as_str().to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn pause_run(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let run = match api.db.get_run(run_id).await {
        Ok(run) => run,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match run.status() {
        RunStatus::Success | RunStatus::Failed | RunStatus::Cancelled => {
            return StatusCode::CONFLICT.into_response();
        }
        RunStatus::Paused => return StatusCode::OK.into_response(),
        _ => {}
    }
    match api
        .db
        .update_run_status(run_id, RunStatus::Paused, None)
        .await
    {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: id.clone(),
                status: RunStatus::Paused.as_str().to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn resume_run(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let run = match api.db.get_run(run_id).await {
        Ok(run) => run,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match run.status() {
        RunStatus::Paused => {}
        RunStatus::Success | RunStatus::Failed | RunStatus::Cancelled => {
            return StatusCode::CONFLICT.into_response();
        }
        _ => return StatusCode::CONFLICT.into_response(),
    }
    let tasks = match api.db.list_tasks(run_id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let new_status = if tasks.is_empty() {
        RunStatus::Pending
    } else {
        RunStatus::Running
    };
    match api.db.update_run_status(run_id, new_status, None).await {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: id.clone(),
                status: new_status.as_str().to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn pause_task(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let task_id = match Uuid::parse_str(&id) {
        Ok(tid) => tid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let (run_id, _) = match api.db.get_task_identity(task_id).await {
        Ok(val) => val,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let tasks = match api.db.list_tasks(run_id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let task = match tasks.into_iter().find(|t| t.id == task_id) {
        Some(task) => task,
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    match task.status() {
        TaskStatus::Pending => {}
        TaskStatus::Paused => return StatusCode::OK.into_response(),
        TaskStatus::Dispatched | TaskStatus::Running => {
            return StatusCode::CONFLICT.into_response();
        }
        _ => return StatusCode::CONFLICT.into_response(),
    }
    match api
        .db
        .update_task_status(task_id, TaskStatus::Paused, None, None)
        .await
    {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::TaskUpdated {
                run_id: run_id.to_string(),
                task_id: id.clone(),
                status: TaskStatus::Paused.as_str().to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn resume_task(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let task_id = match Uuid::parse_str(&id) {
        Ok(tid) => tid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let (run_id, _) = match api.db.get_task_identity(task_id).await {
        Ok(val) => val,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let tasks = match api.db.list_tasks(run_id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let task = match tasks.into_iter().find(|t| t.id == task_id) {
        Some(task) => task,
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    match task.status() {
        TaskStatus::Paused => {}
        _ => return StatusCode::CONFLICT.into_response(),
    }
    match api
        .db
        .update_task_status(task_id, TaskStatus::Pending, None, None)
        .await
    {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::TaskUpdated {
                run_id: run_id.to_string(),
                task_id: id.clone(),
                status: TaskStatus::Pending.as_str().to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Default, Deserialize)]
pub(crate) struct UpdateScheduleRequest {
    schedule: Option<String>,
    enabled: bool,
}

pub(crate) async fn update_workflow_schedule(
    State(api): State<ApiServer>,
    Path(name): Path<String>,
    Json(payload): Json<UpdateScheduleRequest>,
) -> impl IntoResponse {
    let workflow = match load_visible_workflow(&api, &name).await {
        Ok(wf) => wf,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    if api
        .db
        .update_workflow_schedule(workflow.id, payload.schedule.as_deref(), payload.enabled)
        .await
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    StatusCode::OK.into_response()
}
