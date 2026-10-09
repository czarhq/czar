use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use czar_core::database::{
    DeferredJobRepository, NewTask, RunRepository, ScheduleRepository, TaskRepository,
    WorkflowRepository,
};
use czar_core::models::{DeferredJobStatus, RunStatus, TaskStatus};
use serde_json::json;

use super::support::MockD1Server;
use crate::database::uuid_param;

#[tokio::test]
async fn schedules_and_deferred_jobs_are_supported() -> Result<()> {
    let server = MockD1Server::start().await?;
    let db = server.database().await?;
    db.run_migrations().await?;

    let workflow = db
        .create_workflow(
            "scheduled",
            None,
            "job-scheduled",
            "us-east1",
            "proj-2",
            "process",
            None,
            Some("*/5 * * * *"),
        )
        .await?;
    db.update_workflow_schedule(workflow.id, Some("*/5 * * * *"), true)
        .await?;
    db.update_workflow_schedule_times(
        workflow.id,
        Utc::now(),
        Some(Utc::now() - Duration::minutes(1)),
    )
    .await?;

    let due = db.get_due_scheduled_workflows().await?;
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].name, "scheduled");

    let run = db.create_run(workflow.id, "scheduler").await?;
    db.update_run_status(run.id, RunStatus::Running, None)
        .await?;
    db.batch_create_tasks(run.id, 1, "scheduled", "process")
        .await?;
    let mut tasks = db.list_tasks(run.id).await?;
    let task = tasks.remove(0);

    let job = db
        .create_deferred_job(task.id, "bigquery", "job-1", json!({ "query": "select 1" }))
        .await?;
    db.update_deferred_job_status(job.id, DeferredJobStatus::Polling, None)
        .await?;
    db.update_deferred_job_polled(job.id).await?;

    let pending_jobs = db.get_pending_deferred_jobs().await?;
    assert_eq!(pending_jobs.len(), 1);
    assert_eq!(pending_jobs[0].status, DeferredJobStatus::Polling);

    db.complete_deferred_job(job.id).await?;
    let completed = db.get_deferred_jobs_for_task(task.id).await?;
    assert_eq!(completed[0].status, DeferredJobStatus::Completed);

    let second_job = db
        .create_deferred_job(task.id, "cloudrun", "job-2", json!({ "name": "svc" }))
        .await?;
    db.cancel_deferred_jobs_for_task(task.id).await?;
    let jobs = db.get_deferred_jobs_for_task(task.id).await?;
    assert!(
        jobs.iter()
            .any(|entry| entry.id == second_job.id && entry.status == DeferredJobStatus::Cancelled)
    );

    Ok(())
}

#[tokio::test]
async fn retry_windows_and_dependency_failures_affect_task_visibility() -> Result<()> {
    let server = MockD1Server::start().await?;
    let db = server.database().await?;
    db.run_migrations().await?;

    let workflow = db
        .create_workflow(
            "retry-flow",
            Some("Retry flow"),
            "job-retry",
            "us-central1",
            "proj-retry",
            "process",
            None,
            None,
        )
        .await?;
    let run = db.create_run(workflow.id, "manual").await?;
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
                params: json!({}),
                max_retries: 1,
                timeout_seconds: Some(30),
            },
            NewTask {
                task_index: 1,
                task_name: "transform".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: json!({}),
                max_retries: 1,
                timeout_seconds: Some(30),
            },
            NewTask {
                task_index: 2,
                task_name: "retry_me".to_string(),
                executor_type: "process".to_string(),
                depends_on: Vec::new(),
                params: json!({}),
                max_retries: 3,
                timeout_seconds: Some(30),
            },
        ],
    )
    .await?;

    let initial_pending = db.get_pending_tasks_with_workflow(10).await?;
    let initial_names = initial_pending
        .iter()
        .map(|task| task.task_name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(initial_names, vec!["extract", "retry_me"]);

    let tasks = db.list_tasks(run.id).await?;
    let extract = tasks
        .iter()
        .find(|task| task.task_name == "extract")
        .context("missing extract task")?;
    let retry_me = tasks
        .iter()
        .find(|task| task.task_name == "retry_me")
        .context("missing retry task")?;

    db.update_task_status(
        extract.id,
        TaskStatus::Failed,
        Some("exec-extract"),
        Some("boom"),
    )
    .await?;

    let failed = db
        .mark_tasks_failed_by_dependency(run.id, &["extract".to_string()], "upstream failed")
        .await?;
    assert_eq!(failed, vec!["transform".to_string()]);

    let after_failure = db.list_tasks(run.id).await?;
    let transform = after_failure
        .iter()
        .find(|task| task.task_name == "transform")
        .context("missing transform task")?;
    assert_eq!(transform.status, TaskStatus::Failed);
    assert_eq!(transform.error.as_deref(), Some("upstream failed"));

    db.update_task_status(retry_me.id, TaskStatus::Running, Some("exec-retry"), None)
        .await?;
    let retry_at = Utc::now() + Duration::minutes(10);
    db.reset_task_for_retry(retry_me.id, Some("retry later"), Some(retry_at))
        .await?;

    let gated_pending = db.get_pending_tasks_with_workflow(10).await?;
    assert!(
        gated_pending.is_empty(),
        "future retry_at should hide retry_me from pending dispatch"
    );

    db.execute(
        "UPDATE tasks SET retry_at = ? WHERE id = ?",
        vec![
            (Utc::now() - Duration::seconds(1)).to_rfc3339().into(),
            uuid_param(retry_me.id),
        ],
    )
    .await?;

    let unblocked_pending = db.get_pending_tasks_with_workflow(10).await?;
    assert_eq!(unblocked_pending.len(), 1);
    assert_eq!(unblocked_pending[0].task_name, "retry_me");

    let retry_meta = db.get_task_retry_meta(&[retry_me.id]).await?;
    assert_eq!(
        retry_meta.get(&retry_me.id).map(|meta| meta.attempts),
        Some(1)
    );
    assert_eq!(
        retry_meta.get(&retry_me.id).map(|meta| meta.max_retries),
        Some(3)
    );

    Ok(())
}
