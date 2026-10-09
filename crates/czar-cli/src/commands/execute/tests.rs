use super::*;
use czar_core::database::{RunRepository, TaskRepository, WorkflowRepository};
use czar_core::models::json_inner;
use czar_store::SqliteDatabase;
use uuid::Uuid;

#[tokio::test]
async fn test_execute_command_errors_on_missing_workflow_file() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let result = Execute {
        file: "/tmp/does-not-exist-workflow.yaml".to_string(),
        timeout: 1,
        ..Default::default()
    }
    .execute(db)
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_execute_command_success_path() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let temp_dir = std::env::temp_dir().join(format!("czar-execute-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let workflow_path = temp_dir.join("workflow.yaml");
    let config_path = temp_dir.join("orchestrator.yaml");

    std::fs::write(
        &workflow_path,
        r#"
name: wf-execute-success
tasks:
  task_a:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"ok\":true}\n'"
"#,
    )
    .expect("write workflow");
    std::fs::write(
        &config_path,
        r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 100
max_concurrent_dispatches: 10
max_concurrent_status_checks: 50
db_pool_size: 5
enable_triggerer: false
"#,
    )
    .expect("write config");

    let result = Execute {
        file: workflow_path.to_string_lossy().to_string(),
        config: Some(config_path.to_string_lossy().to_string()),
        timeout: 10,
        ..Default::default()
    }
    .execute(db.clone())
    .await;

    assert!(result.is_ok());
    let workflow = db
        .get_workflow("wf-execute-success")
        .await
        .expect("workflow should exist");
    let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status_str(), "success");

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_execute_command_replaces_existing_workflow_name() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    db.create_workflow(
        "wf-execute-replace",
        Some("old"),
        "job",
        "local",
        "local",
        "process",
        None,
        None,
    )
    .await
    .expect("pre-create workflow");

    let temp_dir = std::env::temp_dir().join(format!("czar-execute-replace-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let workflow_path = temp_dir.join("workflow.yaml");
    let config_path = temp_dir.join("orchestrator.yaml");

    std::fs::write(
        &workflow_path,
        r#"
name: wf-execute-replace
tasks:
  step:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"ok\":true}\n'"
"#,
    )
    .expect("write workflow");
    std::fs::write(
        &config_path,
        r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 100
max_concurrent_dispatches: 10
max_concurrent_status_checks: 50
db_pool_size: 5
enable_triggerer: false
"#,
    )
    .expect("write config");

    let result = Execute {
        file: workflow_path.to_string_lossy().to_string(),
        config: Some(config_path.to_string_lossy().to_string()),
        timeout: 10,
        ..Default::default()
    }
    .execute(db.clone())
    .await;
    assert!(result.is_ok());

    let workflows = db.list_workflows().await.expect("list workflows");
    let matching: Vec<_> = workflows
        .into_iter()
        .filter(|w| w.name == "wf-execute-replace")
        .collect();
    assert_eq!(matching.len(), 1);
    // Workflow metadata is preserved, but tasks are updated via snapshots
    assert!(matching[0].current_snapshot_id.is_some());

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_execute_command_returns_error_on_failed_run() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let temp_dir = std::env::temp_dir().join(format!("czar-execute-failed-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let workflow_path = temp_dir.join("workflow.yaml");
    let config_path = temp_dir.join("orchestrator.yaml");

    std::fs::write(
        &workflow_path,
        r#"
name: wf-execute-failed
tasks:
  step:
    executor: process
    command: "exit 1"
"#,
    )
    .expect("write workflow");
    std::fs::write(
        &config_path,
        r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 100
max_concurrent_dispatches: 10
max_concurrent_status_checks: 50
db_pool_size: 5
enable_triggerer: false
"#,
    )
    .expect("write config");

    let err = Execute {
        file: workflow_path.to_string_lossy().to_string(),
        config: Some(config_path.to_string_lossy().to_string()),
        timeout: 10,
        ..Default::default()
    }
    .execute(db)
    .await
    .expect_err("failed run should return error");
    assert!(err.to_string().contains("failed status"));

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_execute_command_keeps_cancelled_run_and_returns_error() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let temp_dir = std::env::temp_dir().join(format!("czar-execute-cancel-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let workflow_path = temp_dir.join("workflow.yaml");
    let config_path = temp_dir.join("orchestrator.yaml");

    std::fs::write(
        &workflow_path,
        r#"
name: wf-execute-cancelled
tasks:
  step:
    executor: process
    command: "sleep 5"
"#,
    )
    .expect("write workflow");
    std::fs::write(
        &config_path,
        r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 100
max_concurrent_dispatches: 10
max_concurrent_status_checks: 50
db_pool_size: 5
enable_triggerer: false
"#,
    )
    .expect("write config");

    let canceller_db = db.clone();
    let canceller = tokio::spawn(async move {
        loop {
            if let Ok(workflow) = canceller_db.get_workflow("wf-execute-cancelled").await
                && let Some(run) = canceller_db
                    .list_runs(Some(workflow.id))
                    .await
                    .expect("list runs")
                    .into_iter()
                    .next()
                && canceller_db
                    .list_tasks(run.id)
                    .await
                    .expect("list tasks")
                    .iter()
                    .any(|task| matches!(task.status(), TaskStatus::Running))
            {
                canceller_db.cancel_run(run.id).await.expect("cancel run");
                return run.id;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    let err = Execute {
        file: workflow_path.to_string_lossy().to_string(),
        config: Some(config_path.to_string_lossy().to_string()),
        timeout: 10,
        ..Default::default()
    }
    .execute(db.clone())
    .await
    .expect_err("cancelled run should return error");
    let run_id = tokio::time::timeout(Duration::from_secs(5), canceller)
        .await
        .expect("run was never cancelled")
        .expect("canceller");

    assert!(err.to_string().contains("cancelled status"), "{err}");
    let run = db.get_run(run_id).await.expect("run");
    assert!(matches!(run.status(), RunStatus::Cancelled));

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_execute_command_without_config_can_timeout() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let temp_dir = std::env::temp_dir().join(format!("czar-execute-timeout-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let workflow_path = temp_dir.join("workflow.yaml");
    std::fs::write(
        &workflow_path,
        r#"
name: wf-execute-timeout
tasks:
  step:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"ok\":true}\n'"
"#,
    )
    .expect("write workflow");

    let err = Execute {
        file: workflow_path.to_string_lossy().to_string(),
        timeout: 0,
        ..Default::default()
    }
    .execute(db)
    .await
    .expect_err("timeout should return error");
    assert!(
        err.to_string()
            .contains("Timeout waiting for workflow to complete")
    );

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_execute_command_with_selector_runs_subset_only() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let temp_dir = std::env::temp_dir().join(format!("czar-execute-select-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let workflow_path = temp_dir.join("workflow.yaml");
    let config_path = temp_dir.join("orchestrator.yaml");

    std::fs::write(
        &workflow_path,
        r#"
name: wf-execute-select
tasks:
  keep:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"ok\":true}\n'"
  drop:
    executor: process
    command: "exit 1"
"#,
    )
    .expect("write workflow");
    std::fs::write(
        &config_path,
        r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 100
max_concurrent_dispatches: 10
max_concurrent_status_checks: 50
db_pool_size: 5
enable_triggerer: false
"#,
    )
    .expect("write config");

    let result = Execute {
        file: workflow_path.to_string_lossy().to_string(),
        config: Some(config_path.to_string_lossy().to_string()),
        selection: crate::commands::selection_spec::FlowSelectionArgs {
            selectors: vec!["keep".to_string()],
            ..Default::default()
        },
        timeout: 10,
        ..Default::default()
    }
    .execute(db.clone())
    .await;
    assert!(result.is_ok());

    let workflow = db
        .get_workflow("wf-execute-select")
        .await
        .expect("workflow should exist");
    let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status_str(), "success");

    let selection = runs[0]
        .task_selection
        .as_ref()
        .map(|value| json_inner(value).clone())
        .expect("task selection should be persisted");
    assert_eq!(selection["mode"], "exact");
    assert_eq!(selection["requested_tasks"], serde_json::json!(["keep"]));
    assert_eq!(selection["resolved_tasks"], serde_json::json!(["keep"]));

    let tasks = db.list_tasks(runs[0].id).await.expect("list tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_name, "keep");
    assert_eq!(tasks[0].status_str(), "success");

    let _ = std::fs::remove_dir_all(temp_dir);
}
