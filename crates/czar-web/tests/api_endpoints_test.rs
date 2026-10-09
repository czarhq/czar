use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use czar_core::database::{
    NewTask, NewWorkflowTask, RunRepository, TaskRepository, WorkflowRepository,
    WorkflowSnapshotRepository,
};
use czar_core::workflow_registry::{WorkflowRegistrationMetadata, with_registration_metadata};
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
    let source_yaml = format!(
        r#"
name: {name}
tasks:
  first:
    executor: process
    command: "echo first"
    input_type: {{}}
    output_type:
      data: str
    inputs: {{}}
  second:
    executor: process
    command: "echo second"
    depends_on: [first]
    input_type:
      first_data: str
    output_type:
      done: bool
    inputs:
      first_data:
        ref: tasks.first.output.data
flows:
  default:
    tasks: [first, second]
  first_only:
    tasks: [first]
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
) -> uuid::Uuid {
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

fn make_temp_project_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("czar-{label}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).expect("temp project dir");
    path
}

#[tokio::test]
async fn test_workflow_endpoints() {
    let (app, db) = setup().await;

    let yaml = r#"
name: api_workflow
tasks:
  first:
    executor: process
    command: "echo first"
    output_type:
      data: str
  second:
    executor: process
    command: "echo second"
    depends_on: [first]
    input_type:
      first_data: str
    output_type:
      done: bool
    inputs:
      first_data:
        ref: tasks.first.output.data
"#;

    let (status, body) = request_json(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({ "yaml": yaml })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "api_workflow");

    let (status, body) =
        request_json(&app, Method::GET, "/api/workflows?limit=10&offset=0", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["total"].as_u64().unwrap_or(0) >= 1);

    let (status, body) = request_json(&app, Method::GET, "/api/workflows/api_workflow", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "api_workflow");
    assert_eq!(body["tasks"].as_array().unwrap().len(), 2);

    let (status, _) = request_json(
        &app,
        Method::PATCH,
        "/api/workflows/api_workflow/schedule",
        Some(json!({ "schedule": "*/5 * * * *", "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let updated = db.get_workflow("api_workflow").await.expect("workflow");
    assert_eq!(updated.schedule.as_deref(), Some("*/5 * * * *"));
    assert!(updated.schedule_enabled);
}

#[tokio::test]
async fn test_workflow_list_hides_stale_and_other_project_registrations() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");

    let current_project = make_temp_project_dir("current-project");
    let current_file = current_project.join("workflow.yaml");
    std::fs::write(&current_file, "name: current\n").expect("current file");

    let other_project = make_temp_project_dir("other-project");
    let other_file = other_project.join("workflow.yaml");
    std::fs::write(&other_file, "name: other\n").expect("other file");

    let stale_file = current_project.join("stale.yaml");
    std::fs::write(&stale_file, "name: stale\n").expect("stale file");

    let current = db
        .create_workflow("current", None, "dag", "local", "local", "dag", None, None)
        .await
        .expect("current workflow");
    db.update_workflow_task_params(
        current.id,
        Some(with_registration_metadata(
            Some(serde_json::json!({"workflow_format": "v2"})),
            &WorkflowRegistrationMetadata::new(&current_project, &current_file),
        )),
    )
    .await
    .expect("current metadata");

    let other = db
        .create_workflow("other", None, "dag", "local", "local", "dag", None, None)
        .await
        .expect("other workflow");
    db.update_workflow_task_params(
        other.id,
        Some(with_registration_metadata(
            Some(serde_json::json!({"workflow_format": "v2"})),
            &WorkflowRegistrationMetadata::new(&other_project, &other_file),
        )),
    )
    .await
    .expect("other metadata");

    let stale = db
        .create_workflow("stale", None, "dag", "local", "local", "dag", None, None)
        .await
        .expect("stale workflow");
    db.update_workflow_task_params(
        stale.id,
        Some(with_registration_metadata(
            Some(serde_json::json!({"workflow_format": "v2"})),
            &WorkflowRegistrationMetadata::new(&current_project, &stale_file),
        )),
    )
    .await
    .expect("stale metadata");
    std::fs::remove_file(&stale_file).expect("remove stale file");

    let manual = db
        .create_workflow("manual", None, "dag", "local", "local", "dag", None, None)
        .await
        .expect("manual workflow");
    assert_eq!(manual.name, "manual");

    let app = build_router(ApiServer::new_with_workflow_definitions_root(
        db,
        current_project.clone(),
    ));

    let (status, body) =
        request_json(&app, Method::GET, "/api/workflows?limit=10&offset=0", None).await;
    assert_eq!(status, StatusCode::OK);

    let names = body["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|item| item["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["current", "manual"]);

    let (status, _) = request_json(&app, Method::GET, "/api/workflows/stale", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_json(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({ "workflow": "other" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_run_list_and_detail_endpoints() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "runs_workflow").await;

    let (status, body) = request_json(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({ "workflow": "runs_workflow" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let run_id = body["run_id"].as_str().unwrap().to_string();
    let run_uuid = Uuid::parse_str(&run_id).expect("run id");

    create_run_with_tasks(&db, &workflow, 2).await;
    db.update_run_status(run_uuid, czar_core::models::RunStatus::Running, None)
        .await
        .expect("run status");

    let (status, body) = request_json(&app, Method::GET, "/api/runs?limit=10&offset=0", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["total"].as_u64().unwrap_or(0) >= 2);

    let (status, body) = request_json(
        &app,
        Method::GET,
        "/api/runs?status=running&limit=10&offset=0",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for item in body["items"].as_array().unwrap() {
        assert_eq!(item["status"], "running");
    }

    let (status, body) = request_json(
        &app,
        Method::GET,
        "/api/runs?workflow=runs_workflow&limit=10&offset=0",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for item in body["items"].as_array().unwrap() {
        assert_eq!(item["workflow"], "runs_workflow");
    }

    let (status, body) = request_json(
        &app,
        Method::GET,
        "/api/runs?search=runs_work&limit=10&offset=0",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["workflow"] == "runs_workflow")
    );

    let (status, body) =
        request_json(&app, Method::GET, &format!("/api/runs/{}", run_id), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["run"]["status"], "running");
}

#[tokio::test]
async fn test_start_run_with_task_selection_stores_resolved_subset() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "selection_workflow").await;

    let (status, body) = request_json(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({
            "workflow": "selection_workflow",
            "selection": {
                "task_names": ["second"],
                "mode": "upstream"
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let run_id = Uuid::parse_str(body["run_id"].as_str().expect("run id")).expect("uuid");
    let run = db.get_run(run_id).await.expect("run");
    let selection = run
        .task_selection
        .as_ref()
        .map(|v| czar_core::models::json_inner(v).clone())
        .expect("task selection");
    assert_eq!(selection["mode"], "upstream");
    assert_eq!(selection["requested_tasks"], json!(["second"]));
    assert_eq!(selection["resolved_tasks"], json!(["first", "second"]));

    // Keep compiler aware this helper still works and workflow exists.
    assert_eq!(workflow.name, "selection_workflow");
}

#[tokio::test]
async fn test_start_run_with_flow_creates_flow_specific_snapshot() {
    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "flow_workflow").await;

    let (status, body) = request_json(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({
            "workflow": "flow_workflow",
            "flow": "first_only"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let run_id = Uuid::parse_str(body["run_id"].as_str().expect("run id")).expect("uuid");
    let run = db.get_run(run_id).await.expect("run");
    let selection = run
        .task_selection
        .as_ref()
        .map(|v| czar_core::models::json_inner(v).clone())
        .expect("task selection");
    assert_eq!(selection["flow_name"], "first_only");
    assert_eq!(selection["resolved_tasks"], json!(["first"]));

    let snapshot_id = run.snapshot_id.expect("snapshot id");
    let snapshot = db.get_snapshot(snapshot_id).await.expect("snapshot");
    let tasks: Vec<czar_core::database::NewWorkflowTask> =
        serde_json::from_value(czar_core::models::json_inner(&snapshot.tasks_json).clone())
            .expect("snapshot tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_name, "first");
}
