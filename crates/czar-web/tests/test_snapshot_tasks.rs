use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

use czar_core::database::{
    RunRepository, TaskRepository, WorkflowRepository, WorkflowSnapshotRepository,
};
use czar_core::task_execution::build_run_tasks_from_snapshot;
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

#[tokio::test]
async fn test_run_with_snapshot_creates_tasks() {
    let (app, db) = setup().await;

    // Create workflow via YAML API (which creates snapshots)
    let yaml = r#"
name: snapshot_workflow
tasks:
  task_a:
    executor: process
    command: "echo task_a"
    output_type:
      result: str
  task_b:
    executor: process
    command: "echo task_b"
    depends_on: [task_a]
    input_type:
      prev_result: str
    output_type:
      result: str
    inputs:
      prev_result:
        ref: tasks.task_a.output.result
"#;

    let (status, workflow_body) = request_json(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({ "yaml": yaml })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Failed to create workflow");
    let _workflow_id = workflow_body["id"].as_str().expect("workflow id");

    // Verify workflow has a snapshot
    let workflow = db
        .get_workflow("snapshot_workflow")
        .await
        .expect("get workflow");
    assert!(
        workflow.current_snapshot_id.is_some(),
        "Workflow should have a snapshot"
    );
    let snapshot_id = workflow.current_snapshot_id.unwrap();

    // Verify snapshot exists
    let snapshot = db.get_snapshot(snapshot_id).await.expect("get snapshot");
    assert_eq!(snapshot.workflow_id, workflow.id);

    // Create a run via API
    let (status, run_body) = request_json(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({ "workflow": "snapshot_workflow" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Failed to create run");
    let run_id = run_body["run_id"].as_str().expect("run id");

    // Verify run has snapshot_id
    let run = db
        .get_run(uuid::Uuid::parse_str(run_id).unwrap())
        .await
        .expect("get run");
    assert!(run.snapshot_id.is_some(), "Run should capture snapshot_id");
    let run_snapshot_id = run.snapshot_id.unwrap();
    let run_snapshot = db
        .get_snapshot(run_snapshot_id)
        .await
        .expect("run snapshot");
    assert_eq!(run_snapshot.workflow_id, workflow.id);

    // Manually create tasks from snapshot (simulating what scheduler does)
    use czar_core::models::json_inner;
    let tasks_json = json_inner(&run_snapshot.tasks_json);
    let snapshot_tasks: Vec<czar_core::database::NewWorkflowTask> =
        serde_json::from_value(tasks_json.clone()).expect("deserialize snapshot tasks");
    let tasks = build_run_tasks_from_snapshot(run.id, &workflow, &snapshot_tasks, None);
    db.batch_create_dag_tasks(run.id, &tasks)
        .await
        .expect("create tasks");

    // Get run details which includes tasks
    let (status, run_detail) =
        request_json(&app, Method::GET, &format!("/api/runs/{}", run_id), None).await;
    assert_eq!(status, StatusCode::OK, "Failed to get run details");

    let tasks = run_detail["tasks"]
        .as_array()
        .expect("tasks should be array");
    assert_eq!(tasks.len(), 2, "Should have 2 tasks created from snapshot");

    // Verify task names match snapshot
    let task_names: Vec<&str> = tasks.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(task_names.contains(&"task_a"));
    assert!(task_names.contains(&"task_b"));

    // Verify task dependencies
    let task_b = tasks.iter().find(|t| t["name"] == "task_b").unwrap();
    let depends_on = task_b["depends_on"].as_array().unwrap();
    assert_eq!(depends_on.len(), 1);
    assert_eq!(depends_on[0], "task_a");
}
