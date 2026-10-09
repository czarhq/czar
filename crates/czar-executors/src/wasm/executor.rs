use std::path::Path;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use tokio::sync::{RwLock, mpsc};
use tracing::warn;
use uuid::Uuid;

use czar_core::executor::{Executor, StatusUpdate};

use super::cli;
#[cfg(feature = "wasm-embedded")]
use super::embedded;

pub struct WasmExecutor {
    pub(super) working_dir: String,
    pub(super) workflow_root: Option<std::path::PathBuf>,
    pub(super) status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
}

impl WasmExecutor {
    pub fn new(working_dir: Option<String>) -> Self {
        Self {
            working_dir: working_dir.unwrap_or_else(|| ".".to_string()),
            workflow_root: None,
            status_tx: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_workflow_root(mut self, workflow_root: std::path::PathBuf) -> Self {
        self.workflow_root = Some(workflow_root);
        self
    }

    pub(super) async fn send_status(&self, update: StatusUpdate) {
        if let Some(tx) = self.status_tx.read().await.as_ref() {
            let _ = tx.send(update);
        }
    }

    async fn execute_module_embedded(
        &self,
        task_id: Uuid,
        module_path: &Path,
        params: Option<serde_json::Value>,
        execution_id: String,
    ) -> Result<String> {
        let entrypoint = params
            .as_ref()
            .and_then(|p| p.get("wasm_entrypoint"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let input_value = params
            .as_ref()
            .and_then(|p| p.get("task_input"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let input_json = serde_json::to_string(&input_value)?;

        self.send_status(StatusUpdate {
            task_id,
            status: "running".to_string(),
            log: Some(format!(
                "S running wasm module (embedded): {}\n",
                module_path.display()
            )),
            output: None,
            error: None,
            progress: None,
        })
        .await;

        let module_path_buf = module_path.to_path_buf();

        let result: Result<serde_json::Value> = tokio::task::spawn_blocking(move || {
            #[cfg(feature = "wasm-embedded")]
            {
                embedded::execute_wasm_embedded(
                    &module_path_buf,
                    &input_json,
                    entrypoint.as_deref(),
                )
            }
            #[cfg(not(feature = "wasm-embedded"))]
            {
                let _ = (&module_path_buf, &input_json, entrypoint);
                Err(anyhow::anyhow!(
                    "wasm embedded mode requires the 'wasm-embedded' feature of czar-executors"
                ))
            }
        })
        .await
        .map_err(|e| anyhow!("Wasm embedded task panicked: {}", e))?;

        match result {
            Ok(output) => {
                self.send_status(StatusUpdate {
                    task_id,
                    status: "success".to_string(),
                    log: None,
                    output: Some(output),
                    error: None,
                    progress: None,
                })
                .await;
                Ok(execution_id)
            }
            Err(e) => {
                warn!("Wasm embedded task failed: {}", e);
                self.send_status(StatusUpdate {
                    task_id,
                    status: "failed".to_string(),
                    log: None,
                    output: None,
                    error: Some(e.to_string()),
                    progress: None,
                })
                .await;
                Err(e)
            }
        }
    }

    async fn execute_module(
        &self,
        task_id: Uuid,
        module_path: &Path,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        let execution_id = Uuid::new_v4().to_string();

        let wasm_mode = params
            .as_ref()
            .and_then(|p| p.get("wasm_mode"))
            .and_then(|v| v.as_str())
            .unwrap_or("embedded");

        if wasm_mode == "embedded" {
            return self
                .execute_module_embedded(task_id, module_path, params, execution_id)
                .await;
        }

        cli::execute_cli(
            task_id,
            module_path,
            params,
            execution_id,
            &self.working_dir,
            self.status_tx.clone(),
        )
        .await
    }
}

impl Default for WasmExecutor {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl Executor for WasmExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        _job_name: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        let raw_path = params
            .as_ref()
            .and_then(|p| p.get("wasm_module_path"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("Missing wasm_module_path in task parameters"))?;
        let module_path = resolve_module_path(&raw_path, self.workflow_root.as_deref());
        self.execute_module(task_id, &module_path, params).await
    }

    async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>) {
        let mut status_tx = self.status_tx.write().await;
        *status_tx = Some(tx);
    }
}

fn resolve_module_path(raw: &str, workflow_root: Option<&Path>) -> std::path::PathBuf {
    let p = std::path::PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else if let Some(root) = workflow_root {
        root.join(p)
    } else {
        p
    }
}
