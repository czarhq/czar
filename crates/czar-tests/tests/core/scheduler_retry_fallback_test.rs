use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use czar_core::config::OrchestratorConfig;
use czar_core::database::{NewTask, RunRepository, TaskRepository, WorkflowRepository};
use czar_core::executor::{Executor, StatusUpdate};
use czar_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use czar_core::models::Workflow;
use czar_core::scheduler::Scheduler;
use czar_store::SqliteDatabase;
use std::sync::Arc;
use tokio::time::{Duration, sleep, timeout};
use uuid::Uuid;

#[derive(Default)]
struct AlwaysFailExecutor;

#[async_trait]
impl Executor for AlwaysFailExecutor {
    async fn execute(
        &self,
        _task_id: Uuid,
        _job_name: &str,
        _params: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        Err(anyhow::anyhow!("mock dispatch failure"))
    }

    async fn set_status_channel(&self, _tx: tokio::sync::mpsc::UnboundedSender<StatusUpdate>) {}
}

#[derive(Default)]
struct AlwaysFailExecutorManager;

#[async_trait]
impl ExecutorManagerTrait for AlwaysFailExecutorManager {
    async fn get_executor(
        &self,
        _executor_type: &str,
        _workflow: &Workflow,
    ) -> anyhow::Result<Arc<dyn Executor>> {
        Ok(Arc::new(AlwaysFailExecutor))
    }
}

fn init_tracing() -> tracing::dispatcher::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::set_default(subscriber)
}

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig::fast_test()
}

async fn setup_db() -> Arc<SqliteDatabase> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
    db.run_migrations().await.expect("migrations");
    db
}

async fn create_running_run_with_retry_task(db: &SqliteDatabase) -> Result<Uuid> {
    let workflow = db
        .create_workflow(
            "retry-reset-fails",
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
            max_retries: 2,
            timeout_seconds: Some(10),
        }],
    )
    .await?;
    db.update_run_status(run.id, czar_core::models::RunStatus::Running, None)
        .await?;
    Ok(run.id)
}

async fn create_pending_run_for_legacy_workflow(db: &SqliteDatabase) -> Result<Uuid> {
    let workflow = db
        .create_workflow(
            "run-status-update-fails",
            None,
            "job",
            "local",
            "local",
            "process",
            Some(serde_json::json!({"task_count": 1})),
            None,
        )
        .await?;
    let run = db.create_run(workflow.id, "test").await?;
    Ok(run.id)
}

#[tokio::test]
async fn test_scheduler_failed_retry_fallback_marks_task_failed() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let run_id = create_running_run_with_retry_task(db.as_ref()).await?;

    sqlx::query(
        r#"
        CREATE TRIGGER fail_retry_reset
        BEFORE UPDATE ON tasks
        WHEN NEW.status = 'pending'
        BEGIN
            SELECT RAISE(ABORT, 'retry reset blocked');
        END;
        "#,
    )
    .execute(db.pool())
    .await?;

    let scheduler = Scheduler::new_with_config(
        db.clone(),
        Arc::new(AlwaysFailExecutorManager),
        test_config(),
    );
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            let run = db.get_run(run_id).await?;
            if run.status_str() == "failed" {
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
        "timed out waiting for fallback failed status after retry reset error"
    );
    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task");
    assert_eq!(task.status_str(), "failed");
    Ok(())
}

#[tokio::test]
async fn test_scheduler_logs_when_run_status_update_to_running_fails() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let run_id = create_pending_run_for_legacy_workflow(db.as_ref()).await?;

    sqlx::query(
        r#"
        CREATE TRIGGER fail_run_running_update
        BEFORE UPDATE ON runs
        WHEN NEW.status = 'running'
        BEGIN
            SELECT RAISE(ABORT, 'run status update blocked');
        END;
        "#,
    )
    .execute(db.pool())
    .await?;

    let scheduler = Scheduler::new_with_config(
        db.clone(),
        Arc::new(AlwaysFailExecutorManager),
        test_config(),
    );
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            if !db.list_tasks(run_id).await?.is_empty() {
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
        "timed out waiting for task creation when run update fails"
    );
    let run = db.get_run(run_id).await?;
    assert_ne!(
        run.status_str(),
        "running",
        "run should not transition to running when the status update fails"
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_uses_explicit_retry_delay_when_present() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let workflow = db
        .create_workflow(
            "retry-delay-explicit",
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
            params: serde_json::json!({
                "command": "echo hi",
                "retry_delay_seconds": 7
            }),
            max_retries: 2,
            timeout_seconds: Some(10),
        }],
    )
    .await?;
    db.update_run_status(run.id, czar_core::models::RunStatus::Running, None)
        .await?;

    let scheduler = Scheduler::new_with_config(
        db.clone(),
        Arc::new(AlwaysFailExecutorManager),
        test_config(),
    );
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let task = timeout(Duration::from_secs(2), async {
        loop {
            let task = db
                .list_tasks(run.id)
                .await?
                .into_iter()
                .next()
                .expect("task exists");
            if task.retry_at.is_some() && task.attempts >= 1 {
                break Ok::<_, anyhow::Error>(task);
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    handle.abort();

    let retry_at = task.retry_at.expect("retry_at should be set");
    let delay_seconds = retry_at.signed_duration_since(Utc::now()).num_seconds();
    assert!(
        (5..=8).contains(&delay_seconds),
        "expected retry delay near 7 seconds, got {delay_seconds}"
    );
    Ok(())
}
