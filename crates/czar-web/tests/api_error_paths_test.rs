use axum::body::{Body, Bytes, to_bytes};
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use czar_core::database::{
    NewTask, NewWorkflowTask, RunRepository, TaskRepository, WorkflowRepository,
    WorkflowSnapshotRepository,
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

async fn request_raw(
    app: &axum::Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Bytes) {
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
    (status, bytes)
}

async fn create_workflow_with_tasks(
    db: &SqliteDatabase,
    name: &str,
) -> czar_core::models::Workflow {
    let workflow = db
        .create_workflow(name, None, "dag", "local", "local", "dag", None, None)
        .await
        .expect("workflow");
    let tasks = vec![NewWorkflowTask {
        task_index: 0,
        task_name: "first".to_string(),
        executor_type: "process".to_string(),
        depends_on: vec![],
        params: json!({"command": "echo first"}),
        signature: None,
    }];
    db.create_workflow_tasks(workflow.id, &tasks)
        .await
        .expect("workflow tasks");
    let source_yaml = format!(
        r#"
name: {name}
tasks:
  first:
    executor: process
    command: "echo first"
"#
    );
    let snapshot = db
        .create_or_get_snapshot(
            workflow.id,
            &format!("snapshot-{name}"),
            serde_json::to_value(&tasks).expect("tasks json"),
            Some(source_yaml),
        )
        .await
        .expect("snapshot");
    db.update_workflow_snapshot(workflow.id, snapshot.id)
        .await
        .expect("update snapshot");
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
async fn test_create_workflow_error_paths_and_replace_conflict() {
    let (app, _db) = setup().await;

    let (status, _) = request_raw(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({ "yaml": "name: bad\ntasks: [" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let invalid_workflow_yaml = r#"
name: invalid_workflow
tasks:
  missing_command:
    executor: process
"#;
    let (status, body) = request_raw(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({ "yaml": invalid_workflow_yaml })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("requires a `command` or `file` path"));

    let valid_yaml = r#"
name: duplicate_workflow
tasks:
  one:
    executor: process
    command: "echo one"
"#;
    let (status, _body) = request_json(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({ "yaml": valid_yaml })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = request_raw(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({ "yaml": valid_yaml, "replace": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("Workflow already exists"));

    let (status, _body) = request_json(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({ "yaml": valid_yaml, "replace": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let compile_error_yaml = r#"
name: missing_file_workflow
tasks:
  one:
    executor: python
    file: does-not-exist.py
"#;
    let (status, body) = request_raw(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({ "yaml": compile_error_yaml })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("does-not-exist.py"));
}

#[tokio::test]
async fn test_run_and_workflow_error_paths() {
    let (app, _db) = setup().await;

    let (status, _) = request_raw(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({ "workflow": "missing" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_raw(&app, Method::GET, "/api/runs/not-a-uuid", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = request_raw(
        &app,
        Method::GET,
        &format!("/api/runs/{}", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_raw(&app, Method::GET, "/api/workflows/missing", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_raw(
        &app,
        Method::PATCH,
        "/api/workflows/missing/schedule",
        Some(json!({ "schedule": "*/1 * * * *", "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_raw(
        &app,
        Method::GET,
        "/api/workflows/missing/task-states",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_run_and_task_control_conflict_paths() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "conflict_workflow").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;
    let task_id = db
        .list_tasks(run_id)
        .await
        .expect("list tasks")
        .into_iter()
        .next()
        .expect("task exists")
        .id;

    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/resume", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/pause", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/pause", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    db.update_run_status(run_id, czar_core::models::RunStatus::Success, None)
        .await
        .expect("mark success");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/pause", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = request_raw(&app, Method::POST, "/api/runs/not-a-uuid/cancel", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/cancel", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_raw(&app, Method::POST, "/api/tasks/not-a-uuid/pause", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/pause", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    db.update_task_status(task_id, czar_core::models::TaskStatus::Running, None, None)
        .await
        .expect("set running");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/pause", task_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    db.update_task_status(task_id, czar_core::models::TaskStatus::Pending, None, None)
        .await
        .expect("set pending");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/resume", task_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_empty_list_and_invalid_uuid_paths() {
    let (app, _db) = setup().await;

    let (status, body) = request_json(&app, Method::GET, "/api/runs?limit=5&offset=0", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert_eq!(body["has_more"], false);
    assert_eq!(body["items"].as_array().expect("runs items").len(), 0);

    let (status, body) =
        request_json(&app, Method::GET, "/api/workflows?limit=5&offset=0", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert_eq!(body["has_more"], false);
    assert_eq!(body["items"].as_array().expect("workflow items").len(), 0);

    let (status, _) = request_raw(&app, Method::POST, "/api/runs/not-a-uuid/pause", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = request_raw(&app, Method::POST, "/api/runs/not-a-uuid/resume", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = request_raw(&app, Method::POST, "/api/tasks/not-a-uuid/resume", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/resume", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_workflows_search_filter_is_case_insensitive() {
    let (app, db) = setup().await;
    let _alpha = create_workflow_with_tasks(&db, "AlphaFlow").await;
    let _beta = create_workflow_with_tasks(&db, "BetaFlow").await;

    let (status, body) = request_json(&app, Method::GET, "/api/workflows?search=alpha", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    let items = body["items"].as_array().expect("workflow items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "AlphaFlow");
}
