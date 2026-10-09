//! Worker executor that calls remote worker HTTP endpoints
//!
//! This executor delegates task execution to a worker container
//! instead of running code locally in the scheduler process.

use anyhow::Result;
use async_trait::async_trait;
use czar_core::executor::{Executor, StatusUpdate};
use czar_core::models::ExecutorType;
use czar_core::worker_client::{ExecuteRequest, ExecuteResponse, WorkerClient};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Executor that delegates to a worker HTTP server
pub struct WorkerExecutor {
    worker_client: Arc<WorkerClient>,
    status_tx: Arc<Mutex<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
}

impl Default for WorkerExecutor {
    fn default() -> Self {
        Self::new("http://127.0.0.1:8081".to_string())
    }
}

impl WorkerExecutor {
    pub fn new(worker_url: String) -> Self {
        let worker_client = Arc::new(WorkerClient::new(worker_url));
        Self {
            worker_client,
            status_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Create from an existing worker client
    pub fn from_client(worker_client: Arc<WorkerClient>) -> Self {
        Self {
            worker_client,
            status_tx: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl Executor for WorkerExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        job_name: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        // Send execute request to worker
        let req = ExecuteRequest {
            task_id,
            task_name: job_name.to_string(),
            executor_type: ExecutorType::Process, // Default to process for now
            params,
        };

        let response = self.worker_client.execute(req).await?;

        // Handle response
        match response {
            ExecuteResponse::Success { output, deferred } => {
                // Send status update if channel is available
                if let Some(tx) = self.status_tx.lock().await.as_ref() {
                    let mut result_output = output.clone();

                    // Include deferrables in output if present
                    if let Some(def) = deferred
                        && let Some(obj) = result_output.as_object_mut()
                    {
                        obj.insert("deferred".to_string(), serde_json::Value::Array(def));
                    }

                    let _ = tx.send(StatusUpdate {
                        task_id,
                        status: "success".to_string(),
                        log: None,
                        output: Some(result_output.clone()),
                        error: None,
                        progress: None,
                    });
                }

                // Return output as JSON string
                Ok(serde_json::to_string(&output)?)
            }
            ExecuteResponse::Failed { error } => {
                // Send failure status update
                if let Some(tx) = self.status_tx.lock().await.as_ref() {
                    let _ = tx.send(StatusUpdate {
                        task_id,
                        status: "failed".to_string(),
                        log: None,
                        output: None,
                        error: Some(error.clone()),
                        progress: None,
                    });
                }

                Err(anyhow::anyhow!("Task failed: {}", error))
            }
        }
    }

    async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>) {
        *self.status_tx.lock().await = Some(tx);
    }
}
