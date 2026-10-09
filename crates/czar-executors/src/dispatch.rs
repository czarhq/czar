use std::path::PathBuf;

use czar_core::executor::{Executor, StatusUpdate};
use czar_core::models::ExecutorType;
use tokio::time::{Duration, timeout};
use uuid::Uuid;

/// Maximum time to wait for a terminal status update after execution completes.
const TERMINAL_UPDATE_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(feature = "cloudrun")]
use std::sync::Arc;

#[cfg(feature = "cloudrun")]
use crate::cloud_run::CloudRunClient;
#[cfg(feature = "library")]
use crate::library::LibraryExecutor;
#[cfg(feature = "process")]
use crate::process::ProcessExecutor;
#[cfg(feature = "process")]
use crate::python::PythonExecutor;
#[cfg(feature = "wasm")]
use crate::wasm::WasmExecutor;

pub async fn dispatch(
    executor_type: ExecutorType,
    working_dir: Option<String>,
    workflow_root: Option<PathBuf>,
    task_id: Uuid,
    task_name: &str,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    match executor_type {
        #[cfg(feature = "process")]
        ExecutorType::Process | ExecutorType::Container => {
            let executor = ProcessExecutor::new(working_dir);
            let command = params
                .as_ref()
                .and_then(|p| p.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or(task_name)
                .to_string();
            execute_with_terminal_updates(&executor, task_id, &command, params).await
        }
        #[cfg(feature = "process")]
        ExecutorType::Python => {
            let executor = PythonExecutor::new(working_dir, workflow_root);
            execute_with_terminal_updates(&executor, task_id, task_name, params).await
        }
        #[cfg(feature = "library")]
        ExecutorType::Library => {
            let executor = LibraryExecutor::new();
            let library_path = params
                .as_ref()
                .and_then(|p| p.get("library_path"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing library_path in task parameters".to_string())?
                .to_string();
            execute_with_terminal_updates(&executor, task_id, &library_path, params).await
        }
        #[cfg(feature = "wasm")]
        ExecutorType::Wasm => {
            let mut executor = WasmExecutor::new(working_dir);
            if let Some(root) = workflow_root {
                executor = executor.with_workflow_root(root);
            }
            execute_with_terminal_updates(&executor, task_id, task_name, params).await
        }
        #[cfg(feature = "cloudrun")]
        ExecutorType::CloudRun => {
            let project = params
                .as_ref()
                .and_then(|p| p.get("project"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let region = params
                .as_ref()
                .and_then(|p| p.get("region"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let client = Arc::new(
                CloudRunClient::new(project, region)
                    .await
                    .map_err(|e| format!("Failed to create CloudRun client: {e}"))?,
            );
            client.clone().start_polling_task();
            execute_with_terminal_updates(&*client, task_id, task_name, params).await
        }
        #[allow(unreachable_patterns)]
        _ => Err(format!(
            "Executor type '{}' is not available in this build",
            executor_type.as_str()
        )),
    }
}

pub async fn execute_with_terminal_updates<E: Executor>(
    executor: &E,
    task_id: Uuid,
    job_name: &str,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;

    executor
        .execute(task_id, job_name, params)
        .await
        .map_err(|e| format!("Execution failed: {e}"))?;

    timeout(
        TERMINAL_UPDATE_TIMEOUT,
        wait_for_terminal_update(task_id, &mut rx),
    )
    .await
    .map_err(|_| "Execution timed out waiting for completion".to_string())?
}

pub async fn wait_for_terminal_update(
    task_id: Uuid,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<StatusUpdate>,
) -> Result<serde_json::Value, String> {
    loop {
        let Some(update) = rx.recv().await else {
            return Err("Execution channel closed before completion".to_string());
        };

        if update.task_id != task_id {
            continue;
        }

        match update.status.as_str() {
            "success" => {
                return update
                    .output
                    .ok_or_else(|| "Execution completed without CZAR_OUTPUT JSON".to_string());
            }
            "failed" => {
                return Err(update
                    .error
                    .unwrap_or_else(|| "Execution failed".to_string()));
            }
            _ => {}
        }
    }
}
