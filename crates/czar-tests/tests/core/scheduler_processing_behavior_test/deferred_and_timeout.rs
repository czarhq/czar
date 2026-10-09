use super::support::*;
use anyhow::Result;
use czar_core::database::{RunRepository, TaskRepository};
use czar_core::scheduler::Scheduler;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::time::{Duration, sleep, timeout};

#[tokio::test]
async fn test_scheduler_deferred_output_creates_deferred_job() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, task_id) =
        create_running_run_with_task(db.as_ref(), "deferred-output", 0, Some(10)).await;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::DeferredSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    timeout(Duration::from_secs(2), async {
        loop {
            if !db.get_deferred_jobs_for_task(task_id).await?.is_empty() {
                break Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    handle.abort();

    let run = db.get_run(run_id).await?;
    assert_eq!(run.status_str(), "running");
    Ok(())
}

#[tokio::test]
async fn test_scheduler_invalid_deferred_payload_is_ignored() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, task_id) =
        create_running_run_with_task(db.as_ref(), "invalid-deferred-output", 0, Some(10)).await;

    let executor = Arc::new(MockExecutor::new(ExecMode::InvalidDeferred));
    let manager = Arc::new(MockExecutorManager {
        executor: executor.clone(),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    timeout(Duration::from_secs(2), async {
        loop {
            if executor.calls.load(Ordering::SeqCst) > 0 {
                break Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    sleep(Duration::from_millis(200)).await;
    handle.abort();

    assert!(db.get_deferred_jobs_for_task(task_id).await?.is_empty());
    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .find(|t| t.id == task_id)
        .expect("task exists");
    assert!(
        matches!(task.status_str(), "dispatched" | "running"),
        "expected task to remain non-terminal, got {}",
        task.status_str()
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_enforces_timeout_for_stuck_task() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, task_id) =
        create_running_run_with_task(db.as_ref(), "timeout-task", 0, Some(1)).await;

    db.update_task_status(
        task_id,
        czar_core::models::TaskStatus::Running,
        Some("exec"),
        None,
    )
    .await?;
    sqlx::query("UPDATE tasks SET started_at = '2000-01-01T00:00:00Z' WHERE id = ?")
        .bind(task_id)
        .execute(db.pool())
        .await?;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    wait_for_run_status(db.as_ref(), run_id, "failed", 2).await?;
    handle.abort();

    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task");
    assert_eq!(task.status_str(), "failed");
    assert!(
        task.error
            .as_deref()
            .unwrap_or_default()
            .contains("timeout")
    );
    Ok(())
}
