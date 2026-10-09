use anyhow::Result;
use std::sync::Arc;
use tokio::time::{Duration, timeout};

use czar_core::config::OrchestratorConfig;
use czar_core::database::{NewTask, RunRepository, TaskRepository, WorkflowRepository};
use czar_core::models::{RunStatus, TaskStatus};
use czar_core::scheduler::Scheduler;
use czar_executors::manager::ExecutorManager;
use czar_store::SqliteDatabase;

fn fast_config() -> OrchestratorConfig {
    OrchestratorConfig::fast_test()
}

/// A run where the condition on task B is never true (false literal via 1 == 2).
/// Task B should be skipped, and the run should reach Success — not stay Running forever.
///
/// This test demonstrates (and will fail before the fix) the bug where skipped tasks
/// are not counted as completed, so the run never transitions out of Running.
#[tokio::test]
async fn test_conditional_task_skipped_run_completes() -> Result<()> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await?);
    db.run_migrations().await?;

    let executor_manager = Arc::new(ExecutorManager::new());

    let workflow = db
        .create_workflow(
            "cond_workflow",
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

    // Task A: always runs and succeeds (echo writes to stdout, output is captured as JSON)
    // Task B: has a condition that is always false — it should be skipped
    let tasks = vec![
        NewTask {
            task_index: 0,
            task_name: "task_a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({
                "command": "echo done"
            }),
            max_retries: 0,
            timeout_seconds: Some(5),
        },
        NewTask {
            task_index: 1,
            task_name: "task_b".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["task_a".to_string()],
            // Condition is always false — task_b should be skipped
            params: serde_json::json!({
                "command": "echo this_should_not_run",
                "condition_expr": "1 == 2"
            }),
            max_retries: 0,
            timeout_seconds: Some(5),
        },
    ];

    db.batch_create_dag_tasks(run.id, &tasks).await?;
    db.update_run_status(run.id, RunStatus::Running, None)
        .await?;

    let scheduler = Scheduler::new_with_config(db.clone(), executor_manager, fast_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    // Wait for the run to reach a terminal status
    let result = timeout(Duration::from_secs(10), async {
        loop {
            let run = db.get_run(run.id).await?;
            if run.status().is_terminal() {
                return Ok::<RunStatus, anyhow::Error>(run.status());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;

    handle.abort();

    let final_status = result
        .expect("timed out — run was stuck in Running (skipped tasks not counted as completed)")
        .expect("db error");

    assert_eq!(
        final_status,
        RunStatus::Success,
        "run should be Success when all non-skipped tasks succeed"
    );

    // Verify task statuses
    let all_tasks = db.list_tasks(run.id).await?;
    let task_a = all_tasks.iter().find(|t| t.task_name == "task_a").unwrap();
    let task_b = all_tasks.iter().find(|t| t.task_name == "task_b").unwrap();

    assert_eq!(
        task_a.status(),
        TaskStatus::Success,
        "task_a should be success"
    );
    assert_eq!(
        task_b.status(),
        TaskStatus::Skipped,
        "task_b condition was false — should be skipped, not pending"
    );

    Ok(())
}

/// A run where ALL tasks are skipped via upstream skip propagation.
/// The run should still reach Success, not stay Running forever.
#[tokio::test]
async fn test_all_tasks_skipped_run_completes() -> Result<()> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await?);
    db.run_migrations().await?;

    let executor_manager = Arc::new(ExecutorManager::new());

    let workflow = db
        .create_workflow(
            "all_skipped_workflow",
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

    // Task A: condition is always false — skipped immediately
    // Task B: depends on A; since A is skipped, B is cascade-skipped
    let tasks = vec![
        NewTask {
            task_index: 0,
            task_name: "task_a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({
                "command": "echo unreachable",
                "condition_expr": "1 == 2"
            }),
            max_retries: 0,
            timeout_seconds: Some(5),
        },
        NewTask {
            task_index: 1,
            task_name: "task_b".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["task_a".to_string()],
            params: serde_json::json!({
                "command": "echo also_unreachable"
            }),
            max_retries: 0,
            timeout_seconds: Some(5),
        },
    ];

    db.batch_create_dag_tasks(run.id, &tasks).await?;
    db.update_run_status(run.id, RunStatus::Running, None)
        .await?;

    let scheduler = Scheduler::new_with_config(db.clone(), executor_manager, fast_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let result = timeout(Duration::from_secs(10), async {
        loop {
            let run = db.get_run(run.id).await?;
            if run.status().is_terminal() {
                return Ok::<RunStatus, anyhow::Error>(run.status());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;

    handle.abort();

    let final_status = result
        .expect("timed out — run stuck in Running when all tasks were skipped")
        .expect("db error");

    assert_eq!(
        final_status,
        RunStatus::Success,
        "run with all tasks skipped should be Success"
    );

    let all_tasks = db.list_tasks(run.id).await?;
    for task in &all_tasks {
        assert_eq!(
            task.status(),
            TaskStatus::Skipped,
            "task {} should be skipped",
            task.task_name
        );
    }

    Ok(())
}
