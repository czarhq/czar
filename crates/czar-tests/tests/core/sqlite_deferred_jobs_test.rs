use anyhow::Result;
use czar_core::database::{NewTask, RunRepository, TaskRepository, WorkflowRepository};
use czar_store::SqliteDatabase;

async fn setup_db_with_task() -> Result<(SqliteDatabase, uuid::Uuid)> {
    let db = SqliteDatabase::new(":memory:").await?;
    db.run_migrations().await?;

    let workflow = db
        .create_workflow("wf", None, "job", "local", "local", "process", None, None)
        .await?;
    let run = db.create_run(workflow.id, "test").await?;
    db.batch_create_dag_tasks(
        run.id,
        &[NewTask {
            task_index: 0,
            task_name: "task-a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo hi"}),
            max_retries: 0,
            timeout_seconds: Some(10),
        }],
    )
    .await?;
    let task_id = db
        .list_tasks(run.id)
        .await?
        .into_iter()
        .next()
        .expect("task should exist")
        .id;
    Ok((db, task_id))
}

#[tokio::test]
async fn test_deferred_job_lifecycle_sqlite_backend() -> Result<()> {
    let (db, task_id) = setup_db_with_task().await?;

    let job = db
        .create_deferred_job(
            task_id,
            "custom_http",
            "job-1",
            serde_json::json!({"url":"https://example.com/status"}),
        )
        .await?;
    assert_eq!(job.status_str(), "pending");

    let pending = db.get_pending_deferred_jobs().await?;
    assert_eq!(pending.len(), 1);

    db.update_deferred_job_status(job.id, czar_core::models::DeferredJobStatus::Polling, None)
        .await?;
    db.update_deferred_job_polled(job.id).await?;
    let task_jobs = db.get_deferred_jobs_for_task(task_id).await?;
    assert_eq!(task_jobs.len(), 1);
    assert_eq!(task_jobs[0].status_str(), "polling");
    assert!(task_jobs[0].last_polled_at.is_some());

    db.complete_deferred_job(job.id).await?;
    let completed = db.get_deferred_jobs_for_task(task_id).await?;
    assert_eq!(completed[0].status_str(), "completed");
    assert!(completed[0].finished_at.is_some());

    let second = db
        .create_deferred_job(
            task_id,
            "custom_http",
            "job-2",
            serde_json::json!({"url":"https://example.com/status/2"}),
        )
        .await?;
    db.fail_deferred_job(second.id, "boom").await?;
    let failed = db
        .get_deferred_jobs_for_task(task_id)
        .await?
        .into_iter()
        .find(|j| j.id == second.id)
        .expect("failed job should exist");
    assert_eq!(failed.status_str(), "failed");
    assert_eq!(failed.error.as_deref(), Some("boom"));

    Ok(())
}

#[tokio::test]
async fn test_cancel_pending_deferred_jobs_for_task() -> Result<()> {
    let (db, task_id) = setup_db_with_task().await?;

    let job = db
        .create_deferred_job(
            task_id,
            "custom_http",
            "job-3",
            serde_json::json!({"url":"https://example.com/status/3"}),
        )
        .await?;
    db.update_deferred_job_status(job.id, czar_core::models::DeferredJobStatus::Polling, None)
        .await?;

    db.cancel_deferred_jobs_for_task(task_id).await?;
    let after_cancel = db
        .get_deferred_jobs_for_task(task_id)
        .await?
        .into_iter()
        .find(|j| j.id == job.id)
        .expect("cancelled job should exist");
    assert_eq!(after_cancel.status_str(), "cancelled");
    assert!(after_cancel.finished_at.is_some());

    Ok(())
}
