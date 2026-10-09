use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{Duration, timeout};

use czar_core::config::OrchestratorConfig;
use czar_core::database::{RunRepository, TaskRepository, WorkflowRepository};
use czar_core::scheduler::Scheduler;
use czar_executors::manager::ExecutorManager;
use czar_store::SqliteDatabase;

#[tokio::test]
async fn test_dag_execution_order_and_performance() -> Result<()> {
    // Use in-memory SQLite database
    let db = Arc::new(SqliteDatabase::new(":memory:").await?);
    db.run_migrations().await?;

    // Use real process executor
    let executor_manager = Arc::new(ExecutorManager::new());

    // Create workflow with A -> B -> C dependency chain
    let workflow = db
        .create_workflow(
            "test_dag", None, "test", "local", "local", "process", None, None,
        )
        .await?;

    let run = db.create_run(workflow.id, "test").await?;

    // Create tasks with dependencies using simple, fast shell commands
    let tasks = vec![
        czar_core::database::NewTask {
            task_index: 0,
            task_name: "A".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({
                "command": "echo 'Task A'"
            }),
            max_retries: 0,
            timeout_seconds: Some(1),
        },
        czar_core::database::NewTask {
            task_index: 1,
            task_name: "B".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["A".to_string()],
            params: serde_json::json!({
                "command": "echo 'Task B'"
            }),
            max_retries: 0,
            timeout_seconds: Some(1),
        },
        czar_core::database::NewTask {
            task_index: 2,
            task_name: "C".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["B".to_string()],
            params: serde_json::json!({
                "command": "echo 'Task C'"
            }),
            max_retries: 0,
            timeout_seconds: Some(1),
        },
    ];

    db.batch_create_dag_tasks(run.id, &tasks).await?;
    db.update_run_status(run.id, czar_core::models::RunStatus::Running, None)
        .await?;

    let config = OrchestratorConfig::fast_test();

    let scheduler = Scheduler::new_with_config(db.clone(), executor_manager, config);
    let scheduler_handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    // Wait for all tasks to complete
    let start_time = Instant::now();
    let result = timeout(Duration::from_millis(200), async {
        loop {
            let (total, completed, _failed) = db.get_run_task_stats(run.id).await?;
            if completed == total && total > 0 {
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
        "Test timed out waiting for tasks to complete"
    );
    let total_duration = start_time.elapsed();

    // Verify results
    let all_tasks = db.list_tasks(run.id).await?;

    // Assertions
    assert_eq!(all_tasks.len(), 3, "Should have exactly 3 tasks");

    for task in &all_tasks {
        assert_eq!(
            task.status_str(),
            "success",
            "Task {} should be successful, got {}",
            task.task_name,
            task.status_str()
        );
        assert!(
            task.started_at.is_some(),
            "Task {} should have started",
            task.task_name
        );
        assert!(
            task.finished_at.is_some(),
            "Task {} should have finished",
            task.task_name
        );
        assert!(
            task.finished_at.unwrap() >= task.started_at.unwrap(),
            "Task {} finished before starting",
            task.task_name
        );
    }

    // Verify execution order A -> B -> C
    let task_a = all_tasks.iter().find(|t| t.task_name == "A").unwrap();
    let task_b = all_tasks.iter().find(|t| t.task_name == "B").unwrap();
    let task_c = all_tasks.iter().find(|t| t.task_name == "C").unwrap();

    assert!(
        task_b.started_at.unwrap() >= task_a.finished_at.unwrap(),
        "Task B started before Task A finished"
    );

    assert!(
        task_c.started_at.unwrap() >= task_b.finished_at.unwrap(),
        "Task C started before Task B finished"
    );

    // Performance requirement: < 100ms total
    assert!(
        total_duration.as_millis() < 100,
        "Total execution time must be < 100ms, got {}ms",
        total_duration.as_millis()
    );

    Ok(())
}
