use anyhow::Result;
use async_trait::async_trait;
use czar_core::database::{NewTask, RunRepository, TaskRepository, WorkflowRepository};
use czar_core::executor::{Executor, StatusUpdate};
use czar_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use czar_core::models::TaskStatus;
use czar_core::models::Workflow;
use czar_store::SqliteDatabase;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub(super) struct FanoutExecutor {
    tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<StatusUpdate>>>,
    dispatched: Mutex<Vec<serde_json::Value>>,
}

impl FanoutExecutor {
    pub(super) fn new() -> Self {
        Self {
            tx: Mutex::new(None),
            dispatched: Mutex::new(Vec::new()),
        }
    }

    pub(super) async fn dispatched_params(&self) -> Vec<serde_json::Value> {
        self.dispatched.lock().await.clone()
    }
}

#[async_trait]
impl Executor for FanoutExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        _job_name: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        let params = params.unwrap_or_else(|| serde_json::json!({}));
        self.dispatched.lock().await.push(params.clone());

        let output = if let Some(input) = params.get("task_input") {
            input.clone()
        } else if let Some(index) = params
            .get("parallel_index")
            .and_then(|value| value.as_i64())
        {
            serde_json::json!({ "slot": index })
        } else if let Some(task_count) = params.get("task_count").and_then(|value| value.as_i64()) {
            serde_json::json!({ "task_count": task_count })
        } else {
            serde_json::json!({ "ok": true })
        };

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

        Ok(format!("exec-{task_id}"))
    }

    async fn set_status_channel(&self, tx: tokio::sync::mpsc::UnboundedSender<StatusUpdate>) {
        let mut guard = self.tx.lock().await;
        *guard = Some(tx);
    }
}

#[derive(Default)]
pub(super) struct FanoutExecutorManager {
    pub(super) executor: Arc<FanoutExecutor>,
}

#[async_trait]
impl ExecutorManagerTrait for FanoutExecutorManager {
    async fn get_executor(
        &self,
        _executor_type: &str,
        _workflow: &Workflow,
    ) -> anyhow::Result<Arc<dyn Executor>> {
        Ok(self.executor.clone())
    }
}

#[derive(Default)]
pub(super) struct ConcurrencyExecutor {
    tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<StatusUpdate>>>,
    dispatched: Mutex<Vec<(Uuid, serde_json::Value)>>,
}

impl ConcurrencyExecutor {
    pub(super) fn new() -> Self {
        Self {
            tx: Mutex::new(None),
            dispatched: Mutex::new(Vec::new()),
        }
    }

    pub(super) async fn dispatched_task_names(&self) -> Vec<String> {
        self.dispatched
            .lock()
            .await
            .iter()
            .filter_map(|(_, params)| {
                params
                    .get("task_name")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string())
            })
            .collect()
    }

    pub(super) async fn complete(&self, task_id: Uuid, output: serde_json::Value) {
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
    }
}

#[async_trait]
impl Executor for ConcurrencyExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        _job_name: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        let params = params.unwrap_or_else(|| serde_json::json!({}));
        self.dispatched.lock().await.push((task_id, params));
        Ok(format!("exec-{task_id}"))
    }

    async fn set_status_channel(&self, tx: tokio::sync::mpsc::UnboundedSender<StatusUpdate>) {
        let mut guard = self.tx.lock().await;
        *guard = Some(tx);
    }
}

#[derive(Default)]
pub(super) struct ConcurrencyExecutorManager {
    pub(super) executor: Arc<ConcurrencyExecutor>,
}

#[async_trait]
impl ExecutorManagerTrait for ConcurrencyExecutorManager {
    async fn get_executor(
        &self,
        _executor_type: &str,
        _workflow: &Workflow,
    ) -> anyhow::Result<Arc<dyn Executor>> {
        Ok(self.executor.clone())
    }
}

pub(super) async fn create_runtime_fanout_run(
    db: &SqliteDatabase,
    name: &str,
    tasks: &[NewTask],
) -> Result<(Uuid, Uuid)> {
    let workflow = db
        .create_workflow(name, None, "job", "local", "local", "dag", None, None)
        .await?;
    let run = db.create_run(workflow.id, "test").await?;
    db.batch_create_dag_tasks(run.id, tasks).await?;
    db.update_run_status(run.id, czar_core::models::RunStatus::Running, None)
        .await?;
    Ok((workflow.id, run.id))
}

pub(super) async fn mark_task_success(
    db: &SqliteDatabase,
    run_id: Uuid,
    task_name: &str,
    output: serde_json::Value,
) -> Result<()> {
    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .find(|task| task.task_name == task_name)
        .expect("task exists");
    db.update_task_output(task.id, output).await?;
    db.update_task_status(task.id, TaskStatus::Success, None, None)
        .await?;
    Ok(())
}
