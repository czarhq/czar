use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn};
use uuid::Uuid;

use czar_core::executor::{Executor, StatusUpdate};

mod execution;
mod logs;
mod secret_mounts;
mod support;

use execution::{ProcessExecutionSpec, spawn_process_execution};
use secret_mounts::materialize_secret_files;
use support::build_env_vars;

#[derive(Debug, Clone)]
pub(super) enum ProcessStatus {
    Running,
    Success,
    Failed,
}

pub struct ProcessExecutor {
    working_dir: String,
    process_states: Arc<RwLock<HashMap<String, ProcessStatus>>>,
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
}

impl Default for ProcessExecutor {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ProcessExecutor {
    pub fn new(working_dir: Option<String>) -> Self {
        Self {
            working_dir: working_dir.unwrap_or_else(|| ".".to_string()),
            process_states: Arc::new(RwLock::new(HashMap::new())),
            status_tx: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn execute_process(
        &self,
        task_id: Uuid,
        command: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        let execution_id = Uuid::new_v4().to_string();
        let command = command.to_string();

        let start_msg = format!(
            "Executing process: {} with execution_id: {}",
            command, execution_id
        );
        info!("{}", start_msg);

        let (
            mut env_vars,
            secret_file_mounts,
            task_file,
            config_file,
            task_module,
            task_function,
            custom_executor,
            task_input,
            runner_path,
            python_path,
            task_args,
            container_image,
        ) = build_env_vars(params)?;
        let working_dir_path = PathBuf::from(&self.working_dir);
        let mounted_secret_files =
            match materialize_secret_files(&working_dir_path, &secret_file_mounts, &mut env_vars)
                .await
            {
                Ok(mounted) => mounted,
                Err(err) => {
                    let error_msg = format!(
                        "failed to materialize secret files for task {}: {}",
                        task_id, err
                    );
                    warn!("{}", error_msg);
                    {
                        let mut states = self.process_states.write().await;
                        states.insert(execution_id.clone(), ProcessStatus::Failed);
                    }
                    if let Some(tx) = self.status_tx.read().await.as_ref() {
                        let _ = tx.send(StatusUpdate {
                            task_id,
                            status: "failed".to_string(),
                            log: None,
                            output: None,
                            error: Some(error_msg),
                            progress: None,
                        });
                    }
                    return Ok(execution_id);
                }
            };
        let task_label = env_vars
            .get("task_name")
            .cloned()
            .unwrap_or_else(|| task_id.to_string());
        env_vars.insert("CZAR_TASK_ID".to_string(), task_id.to_string());
        env_vars.insert("CZAR_EXECUTION_NAME".to_string(), execution_id.clone());
        for (source, target) in [
            ("task_name", "CZAR_TASK_NAME"),
            ("workflow_name", "CZAR_WORKFLOW_NAME"),
            ("run_id", "CZAR_RUN_ID"),
            ("parallel_index", "CZAR_PARALLEL_INDEX"),
            ("parallel_count", "CZAR_PARALLEL_COUNT"),
        ] {
            if let Some(value) = env_vars.get(source).cloned() {
                env_vars.insert(target.to_string(), value);
            }
        }

        // Add execution ID to env
        env_vars.insert("EXECUTION_ID".to_string(), execution_id.clone());

        // Mark as running
        {
            let mut states = self.process_states.write().await;
            states.insert(execution_id.clone(), ProcessStatus::Running);
        }

        // Build human-readable description of what will run
        let task_what = if let Some(image) = container_image.as_ref() {
            let command_label = if command.trim().is_empty() {
                "<image-default>".to_string()
            } else {
                command.clone()
            };
            format!("container {image} :: {command_label}")
        } else if let Some(file) = task_file.as_ref() {
            let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            if let Some(func) = task_function.as_ref() {
                format!("{file_name}::{func}")
            } else {
                file_name.to_string()
            }
        } else if let Some(module) = task_module.as_ref() {
            if let Some(func) = task_function.as_ref() {
                format!("{module}::{func}")
            } else {
                module.clone()
            }
        } else {
            command.clone()
        };

        // Clone for the async task
        let exec_id_clone = execution_id.clone();
        let task_label_clone = task_label.clone();
        let states_clone = self.process_states.clone();
        let status_tx_clone = self.status_tx.clone();
        let working_dir = self.working_dir.clone();
        let command = command.clone();
        let attempt_clone = env_vars.get("CZAR_ATTEMPT").cloned();
        let task_args = task_args.clone();
        spawn_process_execution(ProcessExecutionSpec {
            task_id,
            execution_id: exec_id_clone,
            task_label: task_label_clone,
            task_what,
            attempt: attempt_clone,
            working_dir,
            command,
            task_args,
            env_vars,
            custom_executor,
            task_file,
            config_file,
            task_module,
            task_function,
            task_input,
            runner_path,
            python_path,
            mounted_secret_files,
            secret_file_mounts,
            container_image,
            process_states: states_clone,
            status_tx: status_tx_clone,
        });

        Ok(execution_id)
    }
}

// Python runner is now provided by crate::python_runtime::get_run_task_script()

#[async_trait]
impl Executor for ProcessExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        job_name: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        self.execute_process(task_id, job_name, params).await
    }

    async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>) {
        let mut status_tx = self.status_tx.write().await;
        *status_tx = Some(tx);
    }
}

#[cfg(test)]
mod tests;
