use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use czar_core::database::{
    NewTask, NewWorkflowTask, RunRepository, TaskRepository, WorkflowRepository,
};
use czar_store::SqliteDatabase;
use czar_web::api::{ApiServer, build_router};

async fn setup() -> (axum::Router, Arc<SqliteDatabase>) {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");
    let app = build_router(ApiServer::new(db.clone()));
    (app, db)
}

async fn request_json(
    app: &axum::Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    let body = if let Some(payload) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(payload.to_string())
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).expect("request body"))
        .await
        .expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json")
    };
    (status, json)
}

async fn create_workflow_with_tasks(
    db: &SqliteDatabase,
    name: &str,
) -> czar_core::models::Workflow {
    let workflow = db
        .create_workflow(name, None, "dag", "local", "local", "dag", None, None)
        .await
        .expect("workflow");
    let tasks = vec![
        NewWorkflowTask {
            task_index: 0,
            task_name: "first".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: json!({"command": "echo first"}),
            signature: None,
        },
        NewWorkflowTask {
            task_index: 1,
            task_name: "second".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["first".to_string()],
            params: json!({"command": "echo second"}),
            signature: None,
        },
    ];
    db.create_workflow_tasks(workflow.id, &tasks)
        .await
        .expect("workflow tasks");
    workflow
}

async fn create_run_with_tasks(
    db: &SqliteDatabase,
    workflow: &czar_core::models::Workflow,
    task_count: i32,
) -> Uuid {
    let run = db.create_run(workflow.id, "test").await.expect("run");
    let tasks = (0..task_count)
        .map(|idx| NewTask {
            task_index: idx,
            task_name: format!("task_{}", idx),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: json!({"command": "echo ok"}),
            max_retries: 0,
            timeout_seconds: Some(1),
        })
        .collect::<Vec<_>>();
    db.batch_create_dag_tasks(run.id, &tasks)
        .await
        .expect("run tasks");
    run.id
}

#[tokio::test]
async fn test_run_cancel_pause_resume_endpoints() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "control_workflow").await;
    let run_id = create_run_with_tasks(&db, &workflow, 2).await;
    db.update_run_status(run_id, czar_core::models::RunStatus::Running, None)
        .await
        .expect("run status");

    let tasks = db.list_tasks(run_id).await.expect("tasks");
    db.update_task_status(
        tasks[0].id,
        czar_core::models::TaskStatus::Running,
        None,
        None,
    )
    .await
    .expect("task status");

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/runs/{}/pause", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let run = db.get_run(run_id).await.expect("run");
    assert_eq!(run.status_str(), "paused");

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/runs/{}/resume", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let run = db.get_run(run_id).await.expect("run");
    assert_eq!(run.status_str(), "running");

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/runs/{}/cancel", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let run = db.get_run(run_id).await.expect("run");
    assert_eq!(run.status_str(), "cancelled");
    let tasks = db.list_tasks(run_id).await.expect("tasks");
    for task in tasks {
        assert_eq!(task.status_str(), "cancelled");
    }
}

#[tokio::test]
async fn test_task_pause_resume_endpoints() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "task_control").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;

    let task = db
        .list_tasks(run_id)
        .await
        .expect("tasks")
        .into_iter()
        .next()
        .unwrap();

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/pause", task.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let task = db
        .list_tasks(run_id)
        .await
        .expect("tasks")
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(task.status_str(), "paused");

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/resume", task.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let task = db
        .list_tasks(run_id)
        .await
        .expect("tasks")
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(task.status_str(), "pending");
}

#[tokio::test]
async fn test_resume_run_with_no_tasks_sets_pending_status() {
    let (app, db) = setup().await;
    let workflow = db
        .create_workflow(
            "resume_no_tasks",
            None,
            "dag",
            "local",
            "local",
            "dag",
            None,
            None,
        )
        .await
        .expect("workflow");
    let run = db.create_run(workflow.id, "test").await.expect("run");
    db.update_run_status(run.id, czar_core::models::RunStatus::Paused, None)
        .await
        .expect("pause run");

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/runs/{}/resume", run.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let resumed = db.get_run(run.id).await.expect("run");
    assert_eq!(resumed.status_str(), "pending");
}

#[tokio::test]
async fn test_run_detail_includes_task_timestamps_logs_output_and_error() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "detail_fields_workflow").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;
    db.update_run_status(run_id, czar_core::models::RunStatus::Running, None)
        .await
        .expect("set run running");

    let task = db
        .list_tasks(run_id)
        .await
        .expect("tasks")
        .into_iter()
        .next()
        .expect("task exists");

    db.update_task_status(
        task.id,
        czar_core::models::TaskStatus::Dispatched,
        Some("exec-1"),
        None,
    )
    .await
    .expect("set dispatched");
    db.update_task_status(
        task.id,
        czar_core::models::TaskStatus::Running,
        Some("exec-1"),
        None,
    )
    .await
    .expect("set running");
    db.append_task_log(task.id, "line one\n")
        .await
        .expect("append log");
    db.update_task_output(task.id, json!({"value": 42}))
        .await
        .expect("set output");
    db.update_task_status(
        task.id,
        czar_core::models::TaskStatus::Failed,
        Some("exec-1"),
        Some("boom"),
    )
    .await
    .expect("set failed");

    let (status, body) =
        request_json(&app, Method::GET, &format!("/api/runs/{}", run_id), None).await;
    assert_eq!(status, StatusCode::OK);
    let first_task = body["tasks"]
        .as_array()
        .expect("tasks array")
        .first()
        .expect("first task");
    assert_eq!(first_task["status"], "failed");
    assert!(first_task["started_at"].is_string());
    assert!(first_task["finished_at"].is_string());
    assert!(first_task["dispatched_at"].is_string());
    assert_eq!(first_task["output"]["value"], 42);
    assert!(
        first_task["logs"]
            .as_str()
            .unwrap_or_default()
            .contains("line one")
    );
    assert_eq!(first_task["error"], "boom");
}
