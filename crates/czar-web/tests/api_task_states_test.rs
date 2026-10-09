use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use chrono::Utc;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

use czar_core::database::{
    NewWorkflowTask, RunRepository, TaskRepository, WorkflowRepository, WorkflowSnapshotRepository,
};
use czar_core::models::TaskStatus;
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
    let snapshot = db
        .create_or_get_snapshot(
            workflow.id,
            &format!("snapshot-{name}"),
            serde_json::to_value(&tasks).expect("tasks json"),
            None,
        )
        .await
        .expect("snapshot");
    db.update_workflow_snapshot(workflow.id, snapshot.id)
        .await
        .expect("update snapshot");
    workflow
}

#[tokio::test]
async fn test_workflow_task_states_endpoint_returns_persisted_state_details() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "task_state_workflow").await;
    let run = db.create_run(workflow.id, "test").await.expect("run");
    let run_tasks = db
        .list_workflow_tasks(workflow.id)
        .await
        .expect("workflow tasks");
    db.batch_create_dag_tasks(
        run.id,
        &run_tasks
            .into_iter()
            .map(|task| czar_core::database::NewTask {
                task_index: task.task_index,
                task_name: task.task_name,
                executor_type: task.executor_type,
                depends_on: task.depends_on,
                params: json!({"command":"echo ok"}),
                max_retries: 0,
                timeout_seconds: Some(1),
            })
            .collect::<Vec<_>>(),
    )
    .await
    .expect("run tasks");
    let task_id = db
        .list_tasks(run.id)
        .await
        .expect("list tasks")
        .into_iter()
        .next()
        .expect("task")
        .id;

    db.upsert_workflow_task_state(
        workflow.id,
        "first",
        TaskStatus::Success,
        run.id,
        task_id,
        Some(json!({"value": 42})),
        None,
        Some(Utc::now()),
    )
    .await
    .expect("upsert task state");

    let (status, body) = request_json(
        &app,
        Method::GET,
        "/api/workflows/task_state_workflow/task-states",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["workflow"], "task_state_workflow");
    let items = body["items"].as_array().expect("items");
    assert_eq!(items.len(), 2);

    let first = items
        .iter()
        .find(|item| item["task_name"] == "first")
        .expect("first task state");
    assert_eq!(first["last_status"], "success");
    assert_eq!(first["last_run_id"], run.id.to_string());
    assert_eq!(first["last_task_id"], task_id.to_string());
    assert_eq!(first["run_api_path"], format!("/api/runs/{}", run.id));
    assert_eq!(first["output"], json!({"value": 42}));
    assert!(first["success_at"].is_string());
    assert!(first["finished_at"].is_string());
    assert!(first["failure_at"].is_null());

    let second = items
        .iter()
        .find(|item| item["task_name"] == "second")
        .expect("second task state");
    assert!(second["last_status"].is_null());
    assert!(second["last_run_id"].is_null());
    assert!(second["last_task_id"].is_null());
    assert!(second["run_api_path"].is_null());
    assert!(second["output"].is_null());
}
