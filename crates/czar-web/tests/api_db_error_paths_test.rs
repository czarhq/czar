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
async fn test_handlers_and_routes_return_internal_error_when_db_unavailable() {
    let (app, db) = setup().await;

    db.pool().close().await;

    let (status, _) = request_raw(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({
            "yaml": "name: w\ntasks:\n  one:\n    executor: process\n    command: echo one\n"
                .to_string()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (status, _) = request_raw(&app, Method::GET, "/api/runs?limit=10&offset=0", None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (status, _) =
        request_raw(&app, Method::GET, "/api/workflows?limit=10&offset=0", None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (status, _) = request_raw(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({ "workflow": "missing" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_create_workflow_returns_internal_error_when_workflow_tasks_table_missing() {
    let (app, db) = setup().await;
    sqlx::query("DROP TABLE workflow_tasks")
        .execute(db.pool())
        .await
        .expect("drop workflow_tasks");

    let (status, _) = request_raw(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({
            "yaml": "name: broken_wf\ntasks:\n  one:\n    executor: process\n    command: echo one\n"
                .to_string()
        })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_run_detail_returns_internal_error_when_tasks_query_fails() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "detail_error_wf").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;

    sqlx::query("DROP TABLE tasks")
        .execute(db.pool())
        .await
        .expect("drop tasks");

    let (status, _) = request_raw(&app, Method::GET, &format!("/api/runs/{}", run_id), None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_start_run_returns_internal_error_when_run_insert_fails() {
    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "start_run_insert_error").await;

    sqlx::query("DROP TABLE runs")
        .execute(db.pool())
        .await
        .expect("drop runs");

    let (status, _) = request_raw(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({ "workflow": "start_run_insert_error" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_workflow_detail_returns_internal_error_paths() {
    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "workflow_detail_error").await;

    db.pool().close().await;
    let (status, _) = request_raw(
        &app,
        Method::GET,
        "/api/workflows/workflow_detail_error",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "workflow_detail_task_error").await;
    sqlx::query("DROP TABLE workflow_tasks")
        .execute(db.pool())
        .await
        .expect("drop workflow_tasks");
    let (status, _) = request_raw(
        &app,
        Method::GET,
        "/api/workflows/workflow_detail_task_error",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_workflow_task_states_returns_internal_error_paths() {
    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "workflow_task_states_error").await;

    db.pool().close().await;
    let (status, _) = request_raw(
        &app,
        Method::GET,
        "/api/workflows/workflow_task_states_error/task-states",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "workflow_task_states_task_table_error").await;
    sqlx::query("DROP TABLE workflow_tasks")
        .execute(db.pool())
        .await
        .expect("drop workflow_tasks");
    let (status, _) = request_raw(
        &app,
        Method::GET,
        "/api/workflows/workflow_task_states_task_table_error/task-states",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "workflow_task_states_state_table_error").await;
    sqlx::query("DROP TABLE workflow_task_states")
        .execute(db.pool())
        .await
        .expect("drop workflow_task_states");
    let (status, _) = request_raw(
        &app,
        Method::GET,
        "/api/workflows/workflow_task_states_state_table_error/task-states",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_pause_resume_run_internal_error_paths() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "pause_resume_error").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;
    db.update_run_status(run_id, czar_core::models::RunStatus::Paused, None)
        .await
        .expect("pause run");

    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/pause", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/resume", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    sqlx::query("DROP TABLE tasks")
        .execute(db.pool())
        .await
        .expect("drop tasks");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/resume", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_list_runs_returns_internal_error_when_workflow_lookup_fails() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "runs_workflow_lookup_error").await;
    let _run_id = create_run_with_tasks(&db, &workflow, 1).await;

    sqlx::query("ALTER TABLE workflows RENAME TO workflows_broken")
        .execute(db.pool())
        .await
        .expect("rename workflows");
    let (status, _) = request_raw(&app, Method::GET, "/api/runs?limit=10&offset=0", None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_pause_run_internal_error_paths() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "pause_run_internal_error").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;

    sqlx::query(
        r#"
        CREATE TRIGGER fail_pause_update
        BEFORE UPDATE ON runs
        WHEN NEW.status = 'paused'
        BEGIN
            SELECT RAISE(ABORT, 'pause update blocked');
        END;
        "#,
    )
    .execute(db.pool())
    .await
    .expect("create trigger");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/pause", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (app, db) = setup().await;
    sqlx::query("DROP TABLE runs")
        .execute(db.pool())
        .await
        .expect("drop runs");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/pause", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_resume_run_internal_error_paths_for_get_run_and_update() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "resume_run_internal_error").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;
    db.update_run_status(run_id, czar_core::models::RunStatus::Paused, None)
        .await
        .expect("pause run");

    sqlx::query(
        r#"
        CREATE TRIGGER fail_resume_update
        BEFORE UPDATE ON runs
        WHEN NEW.status = 'running'
        BEGIN
            SELECT RAISE(ABORT, 'resume update blocked');
        END;
        "#,
    )
    .execute(db.pool())
    .await
    .expect("create trigger");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/resume", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (app, db) = setup().await;
    sqlx::query("DROP TABLE runs")
        .execute(db.pool())
        .await
        .expect("drop runs");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/resume", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_pause_resume_task_internal_error_paths() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "task_internal_error").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;
    let task_id = db
        .list_tasks(run_id)
        .await
        .expect("list tasks")
        .into_iter()
        .next()
        .expect("task exists")
        .id;

    sqlx::query(
        r#"
        CREATE TRIGGER fail_pause_task_update
        BEFORE UPDATE ON tasks
        WHEN NEW.status = 'paused'
        BEGIN
            SELECT RAISE(ABORT, 'pause task update blocked');
        END;
        "#,
    )
    .execute(db.pool())
    .await
    .expect("create pause trigger");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/pause", task_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    sqlx::query("DROP TRIGGER fail_pause_task_update")
        .execute(db.pool())
        .await
        .expect("drop pause trigger");
    db.update_task_status(task_id, czar_core::models::TaskStatus::Paused, None, None)
        .await
        .expect("pause task directly");
    sqlx::query(
        r#"
        CREATE TRIGGER fail_resume_task_update
        BEFORE UPDATE ON tasks
        WHEN NEW.status = 'pending'
        BEGIN
            SELECT RAISE(ABORT, 'resume task update blocked');
        END;
        "#,
    )
    .execute(db.pool())
    .await
    .expect("create resume trigger");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/resume", task_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (app, db) = setup().await;
    sqlx::query("DROP TABLE tasks")
        .execute(db.pool())
        .await
        .expect("drop tasks");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/pause", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/resume", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_update_workflow_schedule_returns_internal_error_on_update_failure() {
    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "schedule_update_internal_error").await;

    sqlx::query(
        r#"
        CREATE TRIGGER fail_workflow_schedule_update
        BEFORE UPDATE ON workflows
        WHEN NEW.schedule_enabled = 1
        BEGIN
            SELECT RAISE(ABORT, 'schedule update blocked');
        END;
        "#,
    )
    .execute(db.pool())
    .await
    .expect("create trigger");
    let (status, _) = request_raw(
        &app,
        Method::PATCH,
        "/api/workflows/schedule_update_internal_error/schedule",
        Some(json!({
            "schedule": "*/5 * * * *",
            "enabled": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}
