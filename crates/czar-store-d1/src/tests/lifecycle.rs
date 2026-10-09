use anyhow::{Context, Result};
use czar_core::database::{
    NewTask, NewWorkflowTask, RunListQuery, RunRepository, TaskRepository, WorkflowListQuery,
    WorkflowRepository, WorkflowSnapshotRepository,
};
use czar_core::models::{RunStatus, TaskStatus};
use serde_json::json;

use super::support::MockD1Server;

#[tokio::test]
async fn run_migrations_is_idempotent_over_mock_d1() -> Result<()> {
    let server = MockD1Server::start().await?;
    let db = server.database().await?;

    db.run_migrations().await?;
    db.run_migrations().await?;

    let workflows = db.list_workflows().await?;
    assert!(workflows.is_empty());

    Ok(())
}

#[tokio::test]
async fn smoke_path_covers_workflows_snapshots_runs_tasks_and_state() -> Result<()> {
    let server = MockD1Server::start().await?;
    let db = server.database().await?;
    db.run_migrations().await?;

    let workflow = db
        .create_workflow(
            "etl",
            Some("Daily ETL"),
            "job-etl",
            "us-central1",
            "proj-1",
            "process",
            Some(json!({ "dataset": "warehouse" })),
            Some("0 * * * *"),
        )
        .await?;

    db.create_workflow_tasks(
        workflow.id,
        &[
            NewWorkflowTask {
                task_index: 0,
                task_name: "extract".to_string(),
                executor_type: "process".to_string(),
                depends_on: Vec::new(),
                params: json!({ "step": "extract" }),
                signature: Some(json!({ "kind": "py" })),
            },
            NewWorkflowTask {
                task_index: 1,
                task_name: "transform".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: json!({ "step": "transform" }),
                signature: None,
            },
        ],
    )
    .await?;

    let snapshot = db
        .create_or_get_snapshot(
            workflow.id,
            "hash-1",
            json!([{ "task_name": "extract" }, { "task_name": "transform" }]),
            Some("name: etl".to_string()),
        )
        .await?;
    db.update_workflow_snapshot(workflow.id, snapshot.id)
        .await?;

    let run = db
        .create_run_with_snapshot_and_selection(
            workflow.id,
            Some(snapshot.id),
            "manual",
            Some(json!({ "mode": "exact", "requested_tasks": ["extract", "transform"] })),
        )
        .await?;
    db.update_run_status(run.id, RunStatus::Running, None)
        .await?;

    db.batch_create_dag_tasks(
        run.id,
        &[
            NewTask {
                task_index: 0,
                task_name: "extract".to_string(),
                executor_type: "process".to_string(),
                depends_on: Vec::new(),
                params: json!({ "step": "extract" }),
                max_retries: 2,
                timeout_seconds: Some(30),
            },
            NewTask {
                task_index: 1,
                task_name: "transform".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: json!({ "step": "transform" }),
                max_retries: 1,
                timeout_seconds: Some(60),
            },
        ],
    )
    .await?;

    let workflow_page = db
        .list_workflows_page(&WorkflowListQuery {
            limit: 10,
            offset: 0,
            search: Some("et".to_string()),
        })
        .await?;
    assert_eq!(workflow_page.total, 1);

    let pending = db.get_pending_tasks_with_workflow(10).await?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].task_name, "extract");

    let mut tasks = db.list_tasks(run.id).await?;
    assert_eq!(tasks.len(), 2);
    let extract = tasks.remove(0);
    let transform = tasks.remove(0);

    db.update_task_status(extract.id, TaskStatus::Running, Some("exec-1"), None)
        .await?;
    db.update_task_progress(
        extract.id,
        json!({ "current": 1, "total": 1, "message": "extract done" }),
    )
    .await?;
    db.update_task_output(extract.id, json!({ "rows": 3 }))
        .await?;
    db.append_task_log(extract.id, "extract ok").await?;
    db.update_task_status(extract.id, TaskStatus::Success, Some("exec-1"), None)
        .await?;

    let refreshed_extract = db
        .list_tasks(run.id)
        .await?
        .into_iter()
        .find(|task| task.id == extract.id)
        .context("missing refreshed extract task")?;
    db.upsert_workflow_task_state(
        workflow.id,
        &refreshed_extract.task_name,
        TaskStatus::Success,
        run.id,
        refreshed_extract.id,
        Some(json!({ "rows": 3 })),
        None,
        refreshed_extract.finished_at,
    )
    .await?;

    let pending_after_extract = db.get_pending_tasks_with_workflow(10).await?;
    assert_eq!(pending_after_extract.len(), 1);
    assert_eq!(pending_after_extract[0].task_name, "transform");

    db.batch_update_task_status(&[
        (transform.id, TaskStatus::Running, Some("exec-2"), None),
        (transform.id, TaskStatus::Success, Some("exec-2"), None),
    ])
    .await?;
    db.update_run_status(run.id, RunStatus::Success, None)
        .await?;

    let run_page = db
        .list_runs_page(&RunListQuery {
            limit: 10,
            offset: 0,
            search: None,
            statuses: vec![RunStatus::Success],
            workflow_names: vec!["etl".to_string()],
            since: None,
            until: None,
        })
        .await?;
    assert_eq!(run_page.total, 1);
    assert_eq!(run_page.items[0].workflow_name.as_deref(), Some("etl"));

    let outputs = db
        .get_task_outputs(run.id, &["extract".to_string(), "transform".to_string()])
        .await?;
    assert_eq!(outputs.get("extract"), Some(&json!({ "rows": 3 })));

    let retry_meta = db.get_task_retry_meta(&[extract.id, transform.id]).await?;
    assert_eq!(
        retry_meta.get(&extract.id).map(|meta| meta.attempts),
        Some(0)
    );
    assert_eq!(
        retry_meta.get(&extract.id).map(|meta| meta.max_retries),
        Some(2)
    );
    assert_eq!(
        retry_meta
            .get(&extract.id)
            .and_then(|meta| meta.retry_delay_seconds),
        None
    );
    assert_eq!(
        retry_meta.get(&transform.id).map(|meta| meta.attempts),
        Some(0)
    );
    assert_eq!(
        retry_meta.get(&transform.id).map(|meta| meta.max_retries),
        Some(1)
    );

    let task_states = db.list_workflow_task_states(workflow.id).await?;
    assert_eq!(task_states.len(), 1);
    assert_eq!(task_states[0].task_name, "extract");

    let workflow_tasks = db.list_workflow_tasks(workflow.id).await?;
    assert_eq!(workflow_tasks.len(), 2);

    let run_stats = db.get_run_task_stats(run.id).await?;
    assert_eq!(run_stats, (2, 2, 0));

    let stored_workflow = db.get_workflow("etl").await?;
    assert_eq!(stored_workflow.current_snapshot_id, Some(snapshot.id));

    let stored_snapshot = db.get_snapshot(snapshot.id).await?;
    assert_eq!(stored_snapshot.source_yaml.as_deref(), Some("name: etl"));

    let stored_run = db.get_run(run.id).await?;
    assert_eq!(stored_run.status, RunStatus::Success);

    let task_identity = db.get_task_identity(transform.id).await?;
    assert_eq!(task_identity, (run.id, "transform".to_string()));
    assert_eq!(db.get_task_run_id(transform.id).await?, run.id);

    Ok(())
}

#[tokio::test]
async fn create_run_inherits_current_snapshot_and_cancel_run_cascades() -> Result<()> {
    let server = MockD1Server::start().await?;
    let db = server.database().await?;
    db.run_migrations().await?;

    let workflow = db
        .create_workflow(
            "cancel-flow",
            Some("Cancel flow"),
            "job-cancel",
            "us-central1",
            "proj-cancel",
            "process",
            None,
            None,
        )
        .await?;
    let snapshot = db
        .create_or_get_snapshot(
            workflow.id,
            "cancel-snapshot",
            json!([{ "task_name": "extract" }]),
            Some("name: cancel-flow".to_string()),
        )
        .await?;
    db.update_workflow_snapshot(workflow.id, snapshot.id)
        .await?;

    let run = db.create_run(workflow.id, "manual").await?;
    assert_eq!(run.snapshot_id, Some(snapshot.id));

    db.update_run_status(run.id, RunStatus::Running, None)
        .await?;
    db.batch_create_dag_tasks(
        run.id,
        &[
            NewTask {
                task_index: 0,
                task_name: "pending_task".to_string(),
                executor_type: "process".to_string(),
                depends_on: Vec::new(),
                params: json!({}),
                max_retries: 0,
                timeout_seconds: Some(30),
            },
            NewTask {
                task_index: 1,
                task_name: "running_task".to_string(),
                executor_type: "process".to_string(),
                depends_on: Vec::new(),
                params: json!({}),
                max_retries: 0,
                timeout_seconds: Some(30),
            },
            NewTask {
                task_index: 2,
                task_name: "done_task".to_string(),
                executor_type: "process".to_string(),
                depends_on: Vec::new(),
                params: json!({}),
                max_retries: 0,
                timeout_seconds: Some(30),
            },
        ],
    )
    .await?;

    let tasks = db.list_tasks(run.id).await?;
    tasks
        .iter()
        .find(|task| task.task_name == "pending_task")
        .context("missing pending_task")?;
    let running_task = tasks
        .iter()
        .find(|task| task.task_name == "running_task")
        .context("missing running_task")?;
    let done_task = tasks
        .iter()
        .find(|task| task.task_name == "done_task")
        .context("missing done_task")?;

    db.update_task_status(
        running_task.id,
        TaskStatus::Running,
        Some("exec-running"),
        None,
    )
    .await?;
    db.update_task_status(done_task.id, TaskStatus::Success, Some("exec-done"), None)
        .await?;

    db.cancel_run(run.id).await?;

    let stored_run = db.get_run(run.id).await?;
    assert_eq!(stored_run.status, RunStatus::Cancelled);

    let refreshed = db.list_tasks(run.id).await?;
    let statuses = refreshed
        .into_iter()
        .map(|task| (task.task_name, task.status))
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(statuses.get("pending_task"), Some(&TaskStatus::Cancelled));
    assert_eq!(statuses.get("running_task"), Some(&TaskStatus::Cancelled));
    assert_eq!(statuses.get("done_task"), Some(&TaskStatus::Success));

    Ok(())
}
