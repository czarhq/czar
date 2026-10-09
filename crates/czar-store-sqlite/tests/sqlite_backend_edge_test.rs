use anyhow::Result;
use chrono::{Duration, Utc};
use czar_core::database::{
    NewTask, NewWorkflowTask, RunRepository, TaskRepository, WorkflowRepository,
    WorkflowSnapshotRepository,
};
use czar_store_sqlite::SqliteDatabase;
use uuid::Uuid;

fn task_id(tasks: &[czar_core::models::Task], name: &str) -> Uuid {
    tasks
        .iter()
        .find(|t| t.task_name == name)
        .unwrap_or_else(|| panic!("missing task: {name}"))
        .id
}

#[tokio::test]
async fn test_sqlite_branch_and_edge_paths() -> Result<()> {
    let db = SqliteDatabase::new(":memory:").await?;
    db.run_migrations().await?;

    let wf = db
        .create_workflow(
            "sqlite-edges",
            Some("edge cases"),
            "job",
            "region",
            "project",
            "dag",
            None,
            None,
        )
        .await?;
    let workflow_id = wf.id;

    db.create_workflow_tasks(
        workflow_id,
        &[NewWorkflowTask {
            task_index: 0,
            task_name: "extract".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo extract"}),
            signature: None,
        }],
    )
    .await?;
    db.create_workflow_tasks(
        workflow_id,
        &[
            NewWorkflowTask {
                task_index: 0,
                task_name: "extract".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo extract"}),
                signature: Some(serde_json::json!({"output":"json"})),
            },
            NewWorkflowTask {
                task_index: 1,
                task_name: "load".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: serde_json::json!({"command":"echo load"}),
                signature: None,
            },
        ],
    )
    .await?;
    let workflow_tasks = db.list_workflow_tasks(workflow_id).await?;
    assert_eq!(workflow_tasks.len(), 2);

    let run = db.create_run(workflow_id, "tests").await?;
    let run_id = run.id;
    db.batch_create_dag_tasks(
        run_id,
        &[
            NewTask {
                task_index: 0,
                task_name: "extract".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo extract"}),
                max_retries: 3,
                timeout_seconds: Some(60),
            },
            NewTask {
                task_index: 1,
                task_name: "load".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: serde_json::json!({"command":"echo load"}),
                max_retries: 3,
                timeout_seconds: Some(60),
            },
        ],
    )
    .await?;

    let pending = db.get_pending_tasks().await?;
    assert!(pending.iter().any(|t| t.task_name == "extract"));

    db.batch_update_task_status(&[]).await?;
    let no_failed = db
        .mark_tasks_failed_by_dependency(run_id, &[], "should not apply")
        .await?;
    assert!(no_failed.is_empty());

    let empty_outputs = db.get_task_outputs(run_id, &[]).await?;
    assert!(empty_outputs.is_empty());
    let empty_retry = db.get_task_retry_meta(&[]).await?;
    assert!(empty_retry.is_empty());

    let tasks = db.list_tasks(run_id).await?;
    let extract_id = task_id(&tasks, "extract");
    let load_id = task_id(&tasks, "load");

    db.update_task_output(extract_id, serde_json::json!({"ok": true}))
        .await?;
    let outputs = db
        .get_task_outputs(run_id, &[String::from("extract")])
        .await?;
    assert_eq!(
        outputs.get("extract"),
        Some(&serde_json::json!({"ok": true}))
    );

    let retry_at = Utc::now() + Duration::seconds(30);
    db.reset_task_for_retry(extract_id, Some("retry"), Some(retry_at))
        .await?;
    db.update_task_status(
        extract_id,
        czar_core::models::TaskStatus::Success,
        Some("exec-edge"),
        None,
    )
    .await?;
    db.update_run_status(run_id, czar_core::models::RunStatus::Running, None)
        .await?;
    let pending_with_satisfied = db.get_pending_tasks_with_workflow(10).await?;
    assert!(pending_with_satisfied.iter().any(|t| t.task_id == load_id));

    let run_missing_dep = db.create_run(workflow_id, "tests").await?;
    db.batch_create_dag_tasks(
        run_missing_dep.id,
        &[NewTask {
            task_index: 1,
            task_name: "load_only".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["extract".to_string()],
            params: serde_json::json!({"command":"echo load-only"}),
            max_retries: 0,
            timeout_seconds: None,
        }],
    )
    .await?;
    db.update_run_status(
        run_missing_dep.id,
        czar_core::models::RunStatus::Running,
        None,
    )
    .await?;
    let pending_with_missing_dep_task = db.get_pending_tasks_with_workflow(50).await?;
    assert!(
        pending_with_missing_dep_task
            .iter()
            .any(|t| t.run_id == run_missing_dep.id && t.task_name == "load_only")
    );

    let deferred = db
        .create_deferred_job(
            extract_id,
            "custom_http",
            "edge-job-1",
            serde_json::json!({"url":"http://example/status"}),
        )
        .await?;
    db.get_deferred_jobs_for_task(extract_id).await?;
    db.cancel_deferred_jobs_for_task(extract_id).await?;
    let deferred_after = db.get_deferred_jobs_for_task(extract_id).await?;
    assert!(
        deferred_after
            .iter()
            .any(|j| j.id == deferred.id && j.status_str() == "cancelled")
    );

    Ok(())
}

#[tokio::test]
async fn test_sqlite_batch_create_dag_tasks_is_idempotent_per_run_index() -> Result<()> {
    let db = SqliteDatabase::new(":memory:").await?;
    db.run_migrations().await?;

    let wf = db
        .create_workflow(
            "sqlite-idempotent-tasks",
            None,
            "job",
            "region",
            "project",
            "dag",
            None,
            None,
        )
        .await?;

    let snapshot = db
        .create_or_get_snapshot(
            wf.id,
            "hash-1",
            serde_json::json!([{
                "task_index": 0,
                "task_name": "extract",
                "executor_type": "process",
                "depends_on": [],
                "params": {"command": "echo extract"},
                "signature": null
            }]),
            None,
        )
        .await?;
    db.update_workflow_snapshot(wf.id, snapshot.id).await?;

    let run = db.create_run(wf.id, "tests").await?;
    let tasks = vec![
        NewTask {
            task_index: 0,
            task_name: "extract".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo extract"}),
            max_retries: 0,
            timeout_seconds: None,
        },
        NewTask {
            task_index: 1,
            task_name: "load".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["extract".to_string()],
            params: serde_json::json!({"command":"echo load"}),
            max_retries: 0,
            timeout_seconds: None,
        },
    ];

    db.batch_create_dag_tasks(run.id, &tasks).await?;
    db.batch_create_dag_tasks(run.id, &tasks).await?;

    let created = db.list_tasks(run.id).await?;
    assert_eq!(created.len(), 2);
    assert_eq!(created.iter().filter(|t| t.task_index == 0).count(), 1);
    assert_eq!(created.iter().filter(|t| t.task_index == 1).count(), 1);

    Ok(())
}
