use super::support::*;
use anyhow::Result;
use chrono::Utc;
use czar_core::database::{NewTask, RunRepository, TaskRepository, WorkflowRepository};
use czar_core::scheduler::Scheduler;
use std::sync::Arc;
use tokio::time::{Duration, sleep, timeout};
use uuid::Uuid;

#[tokio::test]
async fn test_scheduler_runs_task_with_missing_dependency_using_persisted_output() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;

    let workflow = db
        .create_workflow(
            "missing-dep-persisted-output",
            None,
            "job",
            "local",
            "local",
            "dag",
            None,
            None,
        )
        .await?;
    let run = db.create_run(workflow.id, "manual").await?;
    db.batch_create_dag_tasks(
        run.id,
        &[NewTask {
            task_index: 1,
            task_name: "load_only".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["extract".to_string()],
            params: serde_json::json!({
                "task_bindings": {
                    "value": {"ref": "tasks.extract.output.value"}
                }
            }),
            max_retries: 0,
            timeout_seconds: Some(10),
        }],
    )
    .await?;
    db.upsert_workflow_task_state(
        workflow.id,
        "extract",
        czar_core::models::TaskStatus::Success,
        Uuid::new_v4(),
        Uuid::new_v4(),
        Some(serde_json::json!({"value": 42})),
        None,
        Some(Utc::now()),
    )
    .await?;
    db.update_run_status(run.id, czar_core::models::RunStatus::Running, None)
        .await?;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(3), async {
        loop {
            let run_state = db.get_run(run.id).await?;
            if run_state.status_str() == "success" {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(
        wait.is_ok(),
        "timed out waiting for run success with persisted dependency output"
    );
    let task = db
        .list_tasks(run.id)
        .await?
        .into_iter()
        .find(|t| t.task_name == "load_only")
        .expect("task exists");
    assert_eq!(task.status_str(), "success");
    Ok(())
}
