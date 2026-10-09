use super::logs::{send_failure, send_running_log, spawn_stderr_log_task, spawn_stdout_log_task};
use super::support::{
    build_container_command, build_python_command, merge_pythonpath_entries, resolve_command_path,
    rewrite_secret_paths_for_container,
};
use super::{ProcessStatus, secret_mounts::MountedSecretFiles};
use crate::env::SecretFileBinding;
use czar_core::executor::StatusUpdate;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock, mpsc};
use tracing::{info, warn};
use uuid::Uuid;

pub(super) struct ProcessExecutionSpec {
    pub(super) task_id: Uuid,
    pub(super) execution_id: String,
    pub(super) task_label: String,
    pub(super) task_what: String,
    pub(super) attempt: Option<String>,
    pub(super) working_dir: String,
    pub(super) command: String,
    pub(super) task_args: Vec<String>,
    pub(super) env_vars: HashMap<String, String>,
    pub(super) custom_executor: Option<serde_json::Value>,
    pub(super) task_file: Option<PathBuf>,
    pub(super) config_file: Option<PathBuf>,
    pub(super) task_module: Option<String>,
    pub(super) task_function: Option<String>,
    pub(super) task_input: Option<serde_json::Value>,
    pub(super) runner_path: Option<PathBuf>,
    pub(super) python_path: Option<PathBuf>,
    pub(super) mounted_secret_files: MountedSecretFiles,
    pub(super) secret_file_mounts: Vec<SecretFileBinding>,
    pub(super) container_image: Option<String>,
    pub(super) process_states: Arc<RwLock<HashMap<String, ProcessStatus>>>,
    pub(super) status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
}

impl Default for ProcessExecutionSpec {
    fn default() -> Self {
        Self {
            task_id: Uuid::nil(),
            execution_id: String::new(),
            task_label: String::new(),
            task_what: String::new(),
            attempt: None,
            working_dir: ".".to_string(),
            command: String::new(),
            task_args: Vec::new(),
            env_vars: HashMap::new(),
            custom_executor: None,
            task_file: None,
            config_file: None,
            task_module: None,
            task_function: None,
            task_input: None,
            runner_path: None,
            python_path: None,
            mounted_secret_files: MountedSecretFiles::default(),
            secret_file_mounts: Vec::new(),
            container_image: None,
            process_states: Arc::new(RwLock::new(HashMap::new())),
            status_tx: Arc::new(RwLock::new(None)),
        }
    }
}

pub(super) fn spawn_process_execution(spec: ProcessExecutionSpec) {
    tokio::spawn(async move {
        let ProcessExecutionSpec {
            task_id,
            execution_id,
            task_label,
            task_what,
            attempt,
            working_dir,
            command,
            task_args,
            mut env_vars,
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
            process_states,
            status_tx,
        } = spec;

        let container_mode = container_image.is_some();
        if container_mode {
            rewrite_secret_paths_for_container(&working_dir, &secret_file_mounts, &mut env_vars);
        }

        let last_output: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let (mut cmd, uses_uv) = match build_command(
            &working_dir,
            &command,
            &task_args,
            custom_executor.is_some(),
            task_file.is_some(),
            task_module.is_some(),
            runner_path.clone(),
            python_path.as_ref(),
            container_image.as_deref(),
            &env_vars,
            &secret_file_mounts,
        ) {
            Ok(value) => value,
            Err(err) => {
                send_failure(
                    &process_states,
                    &status_tx,
                    task_id,
                    &execution_id,
                    format!(
                        "failed to prepare process for task {} (execution {}): {}",
                        task_label, execution_id, err
                    ),
                )
                .await;
                return;
            }
        };

        if !container_mode {
            configure_command_environment(
                &mut cmd,
                &mut env_vars,
                custom_executor.as_ref(),
                task_file.as_ref(),
                config_file.as_ref(),
                task_module.as_deref(),
                task_function.as_deref(),
                task_input.as_ref(),
                python_path.as_ref(),
            );
        }

        if uses_uv && !env_vars.contains_key("UV_CACHE_DIR") {
            let cache_dir = std::env::var("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("czar-uv-cache");
            let _ = std::fs::create_dir_all(&cache_dir);
            env_vars.insert(
                "UV_CACHE_DIR".to_string(),
                cache_dir.to_string_lossy().to_string(),
            );
        }

        if !container_mode {
            for (key, value) in &env_vars {
                cmd.env(key, value);
            }
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                warn!(
                    "Failed to execute task {} (execution {}): {}",
                    task_label, execution_id, err
                );
                send_failure(
                    &process_states,
                    &status_tx,
                    task_id,
                    &execution_id,
                    format!(
                        "failed to spawn process for task {} (execution {}): {}",
                        task_label, execution_id, err
                    ),
                )
                .await;
                return;
            }
        };

        send_running_log(
            &status_tx,
            task_id,
            &task_label,
            &task_what,
            attempt.as_deref(),
        )
        .await;

        let stdout_handle = spawn_stdout_log_task(
            status_tx.clone(),
            task_id,
            last_output.clone(),
            child.stdout.take(),
        );
        let stderr_handle = spawn_stderr_log_task(status_tx.clone(), task_id, child.stderr.take());

        let status = child
            .wait()
            .await
            .ok()
            .map_or(ProcessStatus::Failed, |exit_status| {
                if exit_status.success() {
                    info!("Process completed successfully: {}", execution_id);
                    ProcessStatus::Success
                } else {
                    warn!("Process failed for task {}: {}", task_label, execution_id);
                    ProcessStatus::Failed
                }
            });

        let _ = stdout_handle.await;
        let _ = stderr_handle.await;
        drop(mounted_secret_files);

        let mut states = process_states.write().await;
        states.insert(execution_id.clone(), status.clone());
        drop(states);

        let tx_guard = status_tx.read().await;
        if let Some(tx) = tx_guard.as_ref() {
            let _ = tx.send(StatusUpdate {
                task_id,
                status: if matches!(status, ProcessStatus::Success) {
                    "success".to_string()
                } else {
                    "failed".to_string()
                },
                log: None,
                output: if matches!(status, ProcessStatus::Success) {
                    last_output.lock().await.clone()
                } else {
                    None
                },
                error: if matches!(status, ProcessStatus::Failed) {
                    Some(format!(
                        "process failed for task {} (execution {})",
                        task_label, execution_id
                    ))
                } else {
                    None
                },
                progress: None,
            });
        }
    });
}

fn build_command(
    working_dir: &str,
    command: &str,
    task_args: &[String],
    custom_executor: bool,
    task_file: bool,
    task_module: bool,
    runner_path: Option<PathBuf>,
    python_path: Option<&PathBuf>,
    container_image: Option<&str>,
    env_vars: &HashMap<String, String>,
    secret_file_mounts: &[SecretFileBinding],
) -> anyhow::Result<(Command, bool)> {
    if let Some(image) = container_image {
        return build_container_command(
            working_dir,
            image,
            if command.trim().is_empty() {
                None
            } else {
                Some(command)
            },
            task_args,
            env_vars,
            secret_file_mounts,
        )
        .map(|cmd| (cmd, false));
    }
    if custom_executor {
        return build_python_command(python_path).map(|(mut python_cmd, uses_uv)| {
            let runner = runner_path
                .or_else(|| crate::python_runtime::get_run_executor_script().ok())
                .unwrap_or_else(|| PathBuf::from("run_python_executor.py"));
            python_cmd.arg(runner);
            (python_cmd, uses_uv)
        });
    }
    if task_file || task_module {
        return build_python_command(python_path).map(|(mut python_cmd, uses_uv)| {
            let runner = runner_path
                .or_else(|| crate::python_runtime::get_run_task_script().ok())
                .unwrap_or_else(|| PathBuf::from("run_python_task.py"));
            python_cmd.arg(runner);
            (python_cmd, uses_uv)
        });
    }
    if !task_args.is_empty() {
        let command_path = resolve_command_path(working_dir, command);
        let mut direct_cmd = Command::new(&command_path);
        direct_cmd.args(task_args);
        direct_cmd.current_dir(working_dir);
        return Ok((direct_cmd, false));
    }

    let command_path = resolve_command_path(working_dir, command);
    let mut shell_cmd = Command::new("sh");
    shell_cmd.arg("-c").arg(&command_path);
    shell_cmd.current_dir(working_dir);
    Ok((shell_cmd, false))
}

fn configure_command_environment(
    cmd: &mut Command,
    env_vars: &mut HashMap<String, String>,
    custom_executor: Option<&serde_json::Value>,
    task_file: Option<&PathBuf>,
    config_file: Option<&PathBuf>,
    task_module: Option<&str>,
    task_function: Option<&str>,
    task_input: Option<&serde_json::Value>,
    python_path: Option<&PathBuf>,
) {
    if let Some(custom_executor) = custom_executor {
        env_vars.insert(
            "CZAR_CUSTOM_EXECUTOR_JSON".to_string(),
            custom_executor.to_string(),
        );
        if let Some(config_file) = config_file {
            env_vars.insert(
                "CZAR_TASK_CONFIG_FILE".to_string(),
                config_file.to_string_lossy().to_string(),
            );
        }
    } else if let Some(task_file) = task_file {
        env_vars.insert(
            "CZAR_TASK_FILE".to_string(),
            task_file.to_string_lossy().to_string(),
        );
    } else if let Some(task_module) = task_module {
        env_vars.insert("CZAR_TASK_MODULE".to_string(), task_module.to_string());
    }

    if let Some(task_function) = task_function {
        env_vars.insert("CZAR_TASK_FUNCTION".to_string(), task_function.to_string());
    }
    if let Some(task_input) = task_input {
        env_vars.insert("CZAR_INPUT_JSON".to_string(), task_input.to_string());
    }
    for (source, target) in [
        ("task_name", "CZAR_TASK_NAME"),
        ("workflow_name", "CZAR_WORKFLOW_NAME"),
        ("run_id", "CZAR_RUN_ID"),
    ] {
        if let Some(value) = env_vars.get(source).cloned() {
            env_vars.insert(target.to_string(), value);
        }
    }
    env_vars
        .entry("PYTHONUNBUFFERED".to_string())
        .or_insert_with(|| "1".to_string());
    cmd.env_remove("VIRTUAL_ENV");
    if let Some(root) = python_path {
        cmd.current_dir(root);
        let inherited_pythonpath = env_vars
            .get("PYTHONPATH")
            .cloned()
            .or_else(|| std::env::var("PYTHONPATH").ok());
        env_vars.insert(
            "PYTHONPATH".to_string(),
            merge_pythonpath_entries(root, inherited_pythonpath.as_deref()),
        );
    }
}
