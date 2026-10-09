use anyhow::Result;
use std::sync::Arc;
use tokio::time::{Duration, timeout};

use czar_core::config::OrchestratorConfig;
use czar_core::database::{RunRepository, TaskRepository, WorkflowRepository};
use czar_core::scheduler::Scheduler;
use czar_executors::manager::ExecutorManager;
use czar_store::SqliteDatabase;

#[tokio::test]
async fn test_run_pause_blocks_dispatch_until_resumed() -> Result<()> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await?);
    db.run_migrations().await?;

    let executor_manager = Arc::new(ExecutorManager::new());

    let workflow = db
        .create_workflow(
            "pause_run",
            None,
            "test",
            "local",
            "local",
            "process",
            None,
            None,
        )
        .await?;
    let run = db.create_run(workflow.id, "test").await?;

    let tasks = vec![czar_core::database::NewTask {
        task_index: 0,
        task_name: "A".to_string(),
        executor_type: "process".to_string(),
        depends_on: vec![],
        params: serde_json::json!({
            "command": "echo 'Task A'"
        }),
        max_retries: 0,
        timeout_seconds: Some(1),
    }];
    db.batch_create_dag_tasks(run.id, &tasks).await?;

    db.update_run_status(run.id, czar_core::models::RunStatus::Paused, None)
        .await?;

    let config = OrchestratorConfig::fast_test();
    let scheduler = Scheduler::new_with_config(db.clone(), executor_manager, config);
    let scheduler_handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let pending = db.list_tasks(run.id).await?;
    assert_eq!(pending.len(), 1);
    assert!(pending.iter().all(|t| t.status_str() == "pending"));
    assert!(pending.iter().all(|t| t.dispatched_at.is_none()));
    assert!(pending.iter().all(|t| t.started_at.is_none()));
    assert!(pending.iter().all(|t| t.finished_at.is_none()));

    db.update_run_status(run.id, czar_core::models::RunStatus::Running, None)
        .await?;

    let result = timeout(Duration::from_millis(300), async {
        loop {
            let (total, completed, _failed) = db.get_run_task_stats(run.id).await?;
            if total > 0 && completed == total {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;

    scheduler_handle.abort();

    assert!(
        result.is_ok(),
        "Timed out waiting for task to complete after resume"
    );
    let all_tasks = db.list_tasks(run.id).await?;
    assert!(all_tasks.iter().all(|t| t.status_str() == "success"));

    Ok(())
}

#[tokio::test]
async fn test_task_pause_blocks_dispatch_until_resumed() -> Result<()> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await?);
    db.run_migrations().await?;

    let executor_manager = Arc::new(ExecutorManager::new());

    let workflow = db
        .create_workflow(
            "pause_task",
            None,
            "test",
            "local",
            "local",
            "process",
            None,
            None,
        )
        .await?;
    let run = db.create_run(workflow.id, "test").await?;

    let tasks = vec![czar_core::database::NewTask {
        task_index: 0,
        task_name: "A".to_string(),
        executor_type: "process".to_string(),
        depends_on: vec![],
        params: serde_json::json!({
            "command": "echo 'Task A'"
        }),
        max_retries: 0,
        timeout_seconds: Some(1),
    }];
    db.batch_create_dag_tasks(run.id, &tasks).await?;
    db.update_run_status(run.id, czar_core::models::RunStatus::Running, None)
        .await?;

    let task = db.list_tasks(run.id).await?.into_iter().next().unwrap();
    db.update_task_status(task.id, czar_core::models::TaskStatus::Paused, None, None)
        .await?;

    let config = OrchestratorConfig::fast_test();
    let scheduler = Scheduler::new_with_config(db.clone(), executor_manager, config);
    let scheduler_handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let paused = db.list_tasks(run.id).await?;
    assert_eq!(paused.len(), 1);
    assert!(paused.iter().all(|t| t.status_str() == "paused"));
    assert!(paused.iter().all(|t| t.dispatched_at.is_none()));
    assert!(paused.iter().all(|t| t.started_at.is_none()));
    assert!(paused.iter().all(|t| t.finished_at.is_none()));

    db.update_task_status(task.id, czar_core::models::TaskStatus::Pending, None, None)
        .await?;

    let result = timeout(Duration::from_millis(300), async {
        loop {
            let (total, completed, _failed) = db.get_run_task_stats(run.id).await?;
            if total > 0 && completed == total {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;

    scheduler_handle.abort();

    assert!(
        result.is_ok(),
        "Timed out waiting for task to complete after resume"
    );
    let all_tasks = db.list_tasks(run.id).await?;
    assert!(all_tasks.iter().all(|t| t.status_str() == "success"));

    Ok(())
}
