use super::*;
use czar_core::database::{RunRepository, TaskRepository, WorkflowRepository};
use czar_core::models::{RunLaunchConfig, json_inner};
use czar_store::SqliteDatabase;
use uuid::Uuid;

#[tokio::test]
async fn test_run_command_with_workflow_file() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let temp_dir = std::env::temp_dir().join(format!("czar-run-workflow-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let workflow_path = temp_dir.join("workflow.yaml");

    std::fs::write(
        &workflow_path,
        r#"
name: wf-run-command-success
tasks:
  task_a:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"ok\":true}\n'"
"#,
    )
    .expect("write workflow");

    let result = Run {
        file: Some(workflow_path.to_string_lossy().to_string()),
        timeout: 10,
        ..Default::default()
    }
    .execute(db)
    .await;

    let _ = std::fs::remove_dir_all(temp_dir);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_run_command_errors_on_missing_workflow_file() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let result = Run {
        file: Some("/tmp/does-not-exist-workflow.yaml".to_string()),
        ..Default::default()
    }
    .execute(db)
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_run_command_with_selector_runs_subset_only() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let temp_dir = std::env::temp_dir().join(format!("czar-run-select-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let workflow_path = temp_dir.join("workflow.yaml");
    let config_path = temp_dir.join("orchestrator.yaml");

    std::fs::write(
        &workflow_path,
        r#"
name: wf-run-select
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

    let result = Run {
        file: Some(workflow_path.to_string_lossy().to_string()),
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
        .get_workflow("wf-run-select")
        .await
        .expect("workflow should exist");
    let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
    assert_eq!(runs.len(), 1);

    let tasks = db.list_tasks(runs[0].id).await.expect("list tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_name, "keep");
    assert_eq!(tasks[0].status_str(), "success");

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_parse_var_override_accepts_name_value() {
    assert_eq!(
        parse_var_override("BUILD_PROFILE=release").unwrap(),
        ("BUILD_PROFILE".to_string(), "release".to_string())
    );
}

#[test]
fn test_parse_var_override_rejects_missing_separator() {
    assert!(parse_var_override("BUILD_PROFILE").is_err());
}

#[test]
fn test_parse_run_input_parses_json_values_and_strings() {
    assert_eq!(
        parse_run_input("limit=5").unwrap(),
        ("limit".to_string(), serde_json::json!(5))
    );
    assert_eq!(
        parse_run_input("enabled=true").unwrap(),
        ("enabled".to_string(), serde_json::json!(true))
    );
    assert_eq!(
        parse_run_input("date=2024-01-15").unwrap(),
        ("date".to_string(), serde_json::json!("2024-01-15"))
    );
}

#[test]
fn test_parse_run_input_rejects_missing_separator() {
    assert!(parse_run_input("missing").is_err());
}

#[test]
fn test_run_default_log_flags_are_disabled() {
    let run = Run::default();
    assert!(!run.silent);
    assert!(!run.verbose);
}

#[test]
fn test_parse_run_status_accepts_known_values() {
    assert!(matches!(
        parse_run_status("failed").expect("parse status"),
        RunStatus::Failed
    ));
}

#[test]
fn test_parse_run_status_rejects_unknown_values() {
    assert!(parse_run_status("definitely-not-a-status").is_err());
}

#[tokio::test]
async fn test_run_start_stores_launch_inputs() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    let workflow = db
        .create_workflow(
            "wf-run-start-inputs",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            None,
        )
        .await
        .expect("create workflow");

    RunStart {
        workflow_name: workflow.name.clone(),
        inputs: vec![
            ("date".to_string(), serde_json::json!("2024-01-15")),
            ("limit".to_string(), serde_json::json!(5)),
        ],
        selection: Default::default(),
    }
    .execute(db.clone())
    .await
    .expect("run start should succeed");

    let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
    assert_eq!(runs.len(), 1);
    let raw = runs[0]
        .task_selection
        .as_ref()
        .map(|value| json_inner(value).clone())
        .expect("launch config json");
    let parsed = RunLaunchConfig::parse(runs[0].task_selection.as_ref()).expect("parse config");
    assert_eq!(raw["inputs"]["date"], "2024-01-15");
    assert_eq!(raw["inputs"]["limit"], 5);
    assert_eq!(
        parsed.inputs,
        Some(serde_json::json!({"date": "2024-01-15", "limit": 5}))
    );
    assert!(parsed.task_selection.is_none());
}
