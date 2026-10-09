use super::*;
use crate::commands::selection_spec::FlowSelectionArgs;
use czar_core::database::{
    NewWorkflowTask, RunRepository, WorkflowRepository, WorkflowSnapshotRepository,
};
use czar_core::models::json_inner;
use czar_store::SqliteDatabase;

#[tokio::test]
async fn test_trigger_command_creates_run() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    let workflow = db
        .create_workflow(
            "wf-trigger",
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

    Trigger {
        workflow_name: "wf-trigger".to_string(),
        selection: Default::default(),
    }
    .execute(db.clone())
    .await
    .expect("trigger command should succeed");

    let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].triggered_by, "manual");
    assert!(runs[0].task_selection.is_none());
}

async fn seed_yaml_workflow(db: &Arc<SqliteDatabase>, name: &str) {
    let workflow = db
        .create_workflow(name, None, "job", "local", "local", "process", None, None)
        .await
        .expect("create workflow");

    let tasks = vec![
        NewWorkflowTask {
            task_index: 0,
            task_name: "first".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({}),
            signature: None,
        },
        NewWorkflowTask {
            task_index: 1,
            task_name: "second".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["first".to_string()],
            params: serde_json::json!({}),
            signature: None,
        },
        NewWorkflowTask {
            task_index: 2,
            task_name: "third".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["second".to_string()],
            params: serde_json::json!({}),
            signature: None,
        },
    ];
    db.create_workflow_tasks(workflow.id, &tasks)
        .await
        .expect("create workflow tasks");

    let source_yaml = format!(
        r#"
name: {name}
tasks:
  first:
    executor: process
    command: "echo first"
    output_type: {{}}
  second:
    executor: process
    command: "echo second"
    depends_on: [first]
    input_type: {{}}
    output_type: {{}}
  third:
    executor: process
    command: "echo third"
    depends_on: [second]
    input_type: {{}}
flows:
  first_only:
    tasks: [first]
"#
    );
    let snapshot = db
        .create_or_get_snapshot(
            workflow.id,
            "seed-snapshot",
            serde_json::to_value(&tasks).expect("serialize tasks"),
            Some(source_yaml),
        )
        .await
        .expect("create snapshot");
    db.update_workflow_snapshot(workflow.id, snapshot.id)
        .await
        .expect("update snapshot");
}

#[tokio::test]
async fn test_trigger_with_task_spec_stores_exact_resolved_selection() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    seed_yaml_workflow(&db, "wf-select").await;

    Trigger {
        workflow_name: "wf-select".to_string(),
        selection: FlowSelectionArgs {
            flow: None,
            selectors: vec!["+second".to_string()],
        },
    }
    .execute(db.clone())
    .await
    .expect("trigger command should succeed");

    let workflow = db.get_workflow("wf-select").await.expect("workflow");
    let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
    assert_eq!(runs.len(), 1);
    let selection = runs[0]
        .task_selection
        .as_ref()
        .map(|v| json_inner(v).clone())
        .expect("task selection");
    assert_eq!(selection["mode"], "exact");
    assert_eq!(selection["requested_tasks"], serde_json::json!(["second"]));
    assert_eq!(
        selection["resolved_tasks"],
        serde_json::json!(["first", "second"])
    );
}

#[tokio::test]
async fn test_trigger_with_downstream_depth_spec() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    seed_yaml_workflow(&db, "wf-downstream").await;

    Trigger {
        workflow_name: "wf-downstream".to_string(),
        selection: FlowSelectionArgs {
            flow: None,
            selectors: vec!["first+2".to_string()],
        },
    }
    .execute(db.clone())
    .await
    .expect("trigger command should succeed");

    let workflow = db.get_workflow("wf-downstream").await.expect("workflow");
    let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
    assert_eq!(runs.len(), 1);
    let selection = runs[0]
        .task_selection
        .as_ref()
        .map(|v| json_inner(v).clone())
        .expect("task selection");
    assert_eq!(selection["requested_tasks"], serde_json::json!(["first"]));
    assert_eq!(
        selection["resolved_tasks"],
        serde_json::json!(["first", "second", "third"])
    );
}

#[tokio::test]
async fn test_trigger_with_flow_only_stores_flow_selection() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    seed_yaml_workflow(&db, "wf-flow").await;

    Trigger {
        workflow_name: "wf-flow".to_string(),
        selection: FlowSelectionArgs {
            flow: Some("first_only".to_string()),
            selectors: Vec::new(),
        },
    }
    .execute(db.clone())
    .await
    .expect("trigger command should succeed");

    let workflow = db.get_workflow("wf-flow").await.expect("workflow");
    let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
    assert_eq!(runs.len(), 1);
    let selection = runs[0]
        .task_selection
        .as_ref()
        .map(|v| json_inner(v).clone())
        .expect("task selection");
    assert_eq!(selection["flow_name"], serde_json::json!("first_only"));
    assert_eq!(selection["resolved_tasks"], serde_json::json!(["first"]));
}

#[tokio::test]
async fn test_trigger_with_bidirectional_selector() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    seed_yaml_workflow(&db, "wf-bidi").await;

    Trigger {
        workflow_name: "wf-bidi".to_string(),
        selection: FlowSelectionArgs {
            flow: None,
            selectors: vec!["+second+1".to_string()],
        },
    }
    .execute(db.clone())
    .await
    .expect("trigger command should succeed");

    let workflow = db.get_workflow("wf-bidi").await.expect("workflow");
    let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
    assert_eq!(runs.len(), 1);
    let selection = runs[0]
        .task_selection
        .as_ref()
        .map(|v| json_inner(v).clone())
        .expect("task selection");
    assert_eq!(selection["requested_tasks"], serde_json::json!(["second"]));
    assert_eq!(
        selection["resolved_tasks"],
        serde_json::json!(["first", "second", "third"])
    );
}
