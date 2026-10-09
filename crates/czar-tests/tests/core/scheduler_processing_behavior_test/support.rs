use anyhow::Result;
use async_trait::async_trait;
use czar_core::config::OrchestratorConfig;
use czar_core::database::{
    NewTask, NewWorkflowTask, RunRepository, TaskRepository, WorkflowRepository,
};
use czar_core::executor::{Executor, StatusUpdate};
use czar_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use czar_core::models::Workflow;
use czar_core::notifications::{
    FailureNotificationsConfig, NotificationsConfig, SlackNotificationConfig,
};
use czar_store::SqliteDatabase;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep, timeout};
use uuid::Uuid;

#[derive(Clone, Copy, Default)]
pub(super) enum ExecMode {
    #[default]
    AlwaysSuccess,
    SuccessWithoutOutput,
    AlwaysFail,
    FailThenSuccess,
    DeferredSuccess,
    InvalidDeferred,
}

#[derive(Default)]
pub(super) struct MockExecutor {
    pub(super) mode: ExecMode,
    pub(super) calls: AtomicUsize,
    tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<StatusUpdate>>>,
}

impl MockExecutor {
    pub(super) fn new(mode: ExecMode) -> Self {
        Self {
            mode,
            calls: AtomicUsize::new(0),
            tx: Mutex::new(None),
        }
    }

    async fn send_success(&self, task_id: Uuid, output: serde_json::Value) -> anyhow::Result<()> {
        let guard = self.tx.lock().await;
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(StatusUpdate {
                task_id,
                status: "success".to_string(),
                log: None,
                output: Some(output),
                error: None,
                progress: None,
            });
        }
        Ok(())
    }
}

#[async_trait]
impl Executor for MockExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        _job_name: &str,
        _params: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match self.mode {
            ExecMode::AlwaysFail => Err(anyhow::anyhow!("mock dispatch failure")),
            ExecMode::FailThenSuccess if call == 0 => Err(anyhow::anyhow!("first attempt fails")),
            ExecMode::DeferredSuccess => {
                self.send_success(
                    task_id,
                    serde_json::json!({
                        "deferred": [{
                            "service_type": "custom_http",
                            "job_id": format!("job-{}", task_id),
                            "url": "http://example.com/status",
                        }]
                    }),
                )
                .await?;
                Ok(format!("exec-{task_id}"))
            }
            ExecMode::InvalidDeferred => {
                self.send_success(
                    task_id,
                    serde_json::json!({
                        "deferred": [{
                            "service_type": "custom_http"
                        }]
                    }),
                )
                .await?;
                Ok(format!("exec-{task_id}"))
            }
            ExecMode::AlwaysSuccess | ExecMode::FailThenSuccess => {
                self.send_success(task_id, serde_json::json!({"ok": true}))
                    .await?;
                Ok(format!("exec-{task_id}"))
            }
            ExecMode::SuccessWithoutOutput => {
                let guard = self.tx.lock().await;
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(StatusUpdate {
                        task_id,
                        status: "success".to_string(),
                        log: None,
                        output: None,
                        error: None,
                        progress: None,
                    });
                }
                Ok(format!("exec-{task_id}"))
            }
        }
    }

    async fn set_status_channel(&self, tx: tokio::sync::mpsc::UnboundedSender<StatusUpdate>) {
        let mut guard = self.tx.lock().await;
        *guard = Some(tx);
    }
}

#[derive(Default)]
pub(super) struct MockExecutorManager {
    pub(super) executor: Arc<MockExecutor>,
}

#[async_trait]
impl ExecutorManagerTrait for MockExecutorManager {
    async fn get_executor(
        &self,
        _executor_type: &str,
        _workflow: &Workflow,
    ) -> anyhow::Result<Arc<dyn Executor>> {
        Ok(self.executor.clone())
    }
}

pub(super) async fn setup_db() -> Arc<SqliteDatabase> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
    db.run_migrations().await.expect("migrations");
    db
}

pub(super) fn init_tracing() -> tracing::dispatcher::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::set_default(subscriber)
}

pub(super) fn test_config() -> OrchestratorConfig {
    OrchestratorConfig::fast_test()
}

pub(super) async fn create_running_run_with_task(
    db: &SqliteDatabase,
    name: &str,
    max_retries: i32,
    timeout_seconds: Option<i32>,
) -> (Uuid, Uuid) {
    let workflow = db
        .create_workflow(name, None, "job", "local", "local", "process", None, None)
        .await
        .expect("workflow");
    let run = db.create_run(workflow.id, "test").await.expect("run");
    db.batch_create_dag_tasks(
        run.id,
        &[NewTask {
            task_index: 0,
            task_name: "task-a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo hi"}),
            max_retries,
            timeout_seconds,
        }],
    )
    .await
    .expect("create task");
    db.update_run_status(run.id, czar_core::models::RunStatus::Running, None)
        .await
        .expect("run running");
    let task_id = db
        .list_tasks(run.id)
        .await
        .expect("list tasks")
        .into_iter()
        .next()
        .expect("task exists")
        .id;
    (run.id, task_id)
}

pub(super) async fn create_pending_run_with_workflow_task_signature(
    db: &SqliteDatabase,
    name: &str,
    signature: serde_json::Value,
) -> Result<Uuid> {
    let workflow = db
        .create_workflow(name, None, "dag", "local", "local", "dag", None, None)
        .await?;
    db.create_workflow_tasks(
        workflow.id,
        &[NewWorkflowTask {
            task_index: 0,
            task_name: "task-a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo hi"}),
            signature: Some(signature),
        }],
    )
    .await?;
    let run = db.create_run(workflow.id, "test").await?;
    Ok(run.id)
}

pub(super) async fn wait_for_run_status(
    db: &SqliteDatabase,
    run_id: Uuid,
    expected: &str,
    timeout_secs: u64,
) -> Result<()> {
    timeout(Duration::from_secs(timeout_secs), async {
        loop {
            let run = db.get_run(run_id).await?;
            if run.status_str() == expected {
                break Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    Ok(())
}

pub(super) fn slack_notifications(webhook_url: String) -> NotificationsConfig {
    NotificationsConfig {
        on_failure: Some(FailureNotificationsConfig {
            email: None,
            slack: Some(SlackNotificationConfig { webhook_url }),
        }),
    }
}
