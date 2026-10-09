use anyhow::Result;
use async_trait::async_trait;
use czar_core::config::OrchestratorConfig;
use czar_core::database::{NewTask, RunRepository, TaskRepository, WorkflowRepository};
use czar_core::executor::{Executor, StatusUpdate};
use czar_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use czar_core::models::{RunStatus, TaskStatus, Workflow};
use czar_core::scheduler::Scheduler;
use czar_store::SqliteDatabase;
use std::sync::Arc;
use tokio::time::{Duration, sleep, timeout};
use uuid::Uuid;

#[derive(Default)]
struct NoopExecutor;

#[async_trait]
impl Executor for NoopExecutor {
    async fn execute(
        &self,
        _task_id: Uuid,
        _job_name: &str,
        _params: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        Ok("noop".to_string())
    }

    async fn set_status_channel(&self, _tx: tokio::sync::mpsc::UnboundedSender<StatusUpdate>) {}
}

#[derive(Default)]
struct NoopExecutorManager;

#[async_trait]
impl ExecutorManagerTrait for NoopExecutorManager {
    async fn get_executor(
        &self,
        _executor_type: &str,
        _workflow: &Workflow,
    ) -> anyhow::Result<Arc<dyn Executor>> {
        Ok(Arc::new(NoopExecutor))
    }
}

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig::fast_test()
}

#[tokio::test]
async fn test_failed_status_error_is_appended_to_task_logs() -> Result<()> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
    db.run_migrations().await.expect("migrations");

    let workflow = db
        .create_workflow(
            "failure-log",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            None,
        )
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
    db.update_run_status(run.id, RunStatus::Running, None)
        .await?;

    let task_id = db
        .list_tasks(run.id)
        .await?
        .into_iter()
        .next()
        .expect("task")
        .id;
    db.update_task_status(task_id, TaskStatus::Running, Some("exec"), None)
        .await?;

    let scheduler = Arc::new(Scheduler::new_with_config(
        db.clone(),
        Arc::new(NoopExecutorManager),
        test_config(),
    ));
    let runner = Arc::clone(&scheduler);
    let handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    let error_message = "python execution requires a pyproject.toml at /tmp/workflow";
    scheduler.enqueue_status_update(StatusUpdate {
        task_id,
        status: "failed".to_string(),
        log: None,
        output: None,
        error: Some(error_message.to_string()),
        progress: None,
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            let task = db
                .list_tasks(run.id)
                .await?
                .into_iter()
                .next()
                .expect("task");
            if task.status() == TaskStatus::Failed {
                let logs = task.logs.unwrap_or_default();
                if logs.contains(error_message) {
                    break;
                }
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(wait.is_ok(), "timed out waiting for failed-task error logs");
    Ok(())
}
