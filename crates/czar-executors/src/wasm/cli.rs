use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock, mpsc};
use tracing::{info, warn};
use uuid::Uuid;

use czar_core::executor::StatusUpdate;

use super::support;

pub async fn execute_cli(
    task_id: Uuid,
    module_path: &Path,
    params: Option<serde_json::Value>,
    execution_id: String,
    working_dir: &str,
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
) -> Result<String> {
    let mut env_vars = support::build_wasm_env(params.as_ref());

    let runtime = params
        .as_ref()
        .and_then(|p| p.get("wasm_runtime"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .or_else(|| std::env::var("CZAR_WASM_RUNTIME").ok())
        .unwrap_or_else(|| "wasmtime".to_string());
    let entrypoint = params
        .as_ref()
        .and_then(|p| p.get("wasm_entrypoint"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string);
    let extra_args: Vec<String> = params
        .as_ref()
        .and_then(|p| p.get("task_args"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    env_vars.insert("EXECUTION_ID".to_string(), execution_id.clone());
    env_vars.insert(
        "CZAR_WASM_MODULE".to_string(),
        module_path.to_string_lossy().to_string(),
    );

    let invocation = if let Some(ep) = entrypoint.as_deref() {
        format!("{} run --invoke {} {}", runtime, ep, module_path.display())
    } else {
        format!("{} run {}", runtime, module_path.display())
    };
    info!("Executing Wasm module: {}", invocation);

    if let Some(tx) = status_tx.read().await.as_ref() {
        let _ = tx.send(StatusUpdate {
            task_id,
            status: "running".to_string(),
            log: Some(format!(
                "S running wasm module: {}\n",
                module_path.display()
            )),
            output: None,
            error: None,
            progress: None,
        });
    }

    let working_dir = working_dir.to_string();
    let module_path = module_path.to_path_buf();

    tokio::spawn(async move {
        spawn_wasm_process(
            task_id,
            &module_path,
            &runtime,
            entrypoint.as_deref(),
            &env_vars,
            &extra_args,
            &working_dir,
            status_tx,
        )
        .await;
    });

    Ok(execution_id)
}

async fn spawn_wasm_process(
    task_id: Uuid,
    module_path: &PathBuf,
    runtime: &str,
    entrypoint: Option<&str>,
    env_vars: &std::collections::HashMap<String, String>,
    extra_args: &[String],
    working_dir: &str,
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
) {
    let mut cmd = Command::new(runtime);
    cmd.current_dir(working_dir);
    cmd.arg("run");
    if let Some(ep) = entrypoint {
        cmd.args(["--invoke", ep]);
    }
    if support::is_wasmtime_runtime(runtime) {
        for (key, value) in env_vars {
            cmd.arg("--env");
            cmd.arg(format!("{key}={value}"));
        }
    }
    cmd.arg(module_path);
    cmd.args(extra_args);
    for (key, value) in env_vars {
        cmd.env(key, value);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let child = cmd.spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(err) => {
            let message = format!(
                "failed to spawn wasm runtime '{}' for {}: {}",
                runtime,
                module_path.display(),
                err
            );
            warn!("{}", message);
            if let Some(tx) = status_tx.read().await.as_ref() {
                let _ = tx.send(StatusUpdate {
                    task_id,
                    status: "failed".to_string(),
                    log: None,
                    output: None,
                    error: Some(message),
                    progress: None,
                });
            }
            return;
        }
    };

    let last_output: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
    let stdout_reader = child.stdout.take().map(BufReader::new);
    let stderr_reader = child.stderr.take().map(BufReader::new);

    let stdout_handle = tokio::spawn(read_stdout(
        task_id,
        stdout_reader,
        status_tx.clone(),
        last_output.clone(),
    ));
    let stderr_handle = tokio::spawn(read_stderr(task_id, stderr_reader, status_tx.clone()));

    let exit_status = child.wait().await;
    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    match exit_status {
        Ok(status) if status.success() => {
            if let Some(tx) = status_tx.read().await.as_ref() {
                let _ = tx.send(StatusUpdate {
                    task_id,
                    status: "success".to_string(),
                    log: None,
                    output: last_output.lock().await.clone(),
                    error: None,
                    progress: None,
                });
            }
        }
        Ok(status) => {
            let message = format!(
                "wasm runtime '{}' exited with status {} for {}",
                runtime,
                status,
                module_path.display()
            );
            warn!("{}", message);
            if let Some(tx) = status_tx.read().await.as_ref() {
                let _ = tx.send(StatusUpdate {
                    task_id,
                    status: "failed".to_string(),
                    log: None,
                    output: None,
                    error: Some(message),
                    progress: None,
                });
            }
        }
        Err(err) => {
            let message = format!(
                "failed while waiting for wasm runtime '{}' on {}: {}",
                runtime,
                module_path.display(),
                err
            );
            warn!("{}", message);
            if let Some(tx) = status_tx.read().await.as_ref() {
                let _ = tx.send(StatusUpdate {
                    task_id,
                    status: "failed".to_string(),
                    log: None,
                    output: None,
                    error: Some(message),
                    progress: None,
                });
            }
        }
    }
}

async fn read_stdout(
    task_id: Uuid,
    reader: Option<BufReader<tokio::process::ChildStdout>>,
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
    last_output: Arc<Mutex<Option<serde_json::Value>>>,
) {
    let Some(mut reader) = reader else {
        return;
    };
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await.unwrap_or_default();
        if bytes_read == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if let Some(json_str) = trimmed.strip_prefix("CZAR_OUTPUT:")
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str.trim())
        {
            *last_output.lock().await = Some(value);
            continue;
        }
        if let Some(json_str) = trimmed.strip_prefix("CZAR_PROGRESS:")
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str.trim())
        {
            if let Some(tx) = status_tx.read().await.as_ref() {
                let _ = tx.send(StatusUpdate {
                    task_id,
                    status: "progress".to_string(),
                    log: None,
                    output: None,
                    error: None,
                    progress: Some(value),
                });
            }
            continue;
        }
        if let Some(tx) = status_tx.read().await.as_ref() {
            let ts = chrono::Utc::now().format("%H:%M:%S%.3f");
            let _ = tx.send(StatusUpdate {
                task_id,
                status: "log".to_string(),
                log: Some(format!("O {ts} {trimmed}\n")),
                output: None,
                error: None,
                progress: None,
            });
        }
    }
}

async fn read_stderr(
    task_id: Uuid,
    reader: Option<BufReader<tokio::process::ChildStderr>>,
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
) {
    let Some(mut reader) = reader else {
        return;
    };
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await.unwrap_or_default();
        if bytes_read == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if let Some(tx) = status_tx.read().await.as_ref() {
            let ts = chrono::Utc::now().format("%H:%M:%S%.3f");
            let _ = tx.send(StatusUpdate {
                task_id,
                status: "log".to_string(),
                log: Some(format!("E {ts} {trimmed}\n")),
                output: None,
                error: None,
                progress: None,
            });
        }
    }
}
