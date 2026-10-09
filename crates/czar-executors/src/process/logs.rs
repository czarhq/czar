use std::collections::HashMap;
use std::sync::Arc;

use czar_core::executor::StatusUpdate;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, RwLock, mpsc};
use tracing::warn;
use uuid::Uuid;

use super::ProcessStatus;

pub(super) async fn send_failure(
    process_states: &Arc<RwLock<HashMap<String, ProcessStatus>>>,
    status_tx: &Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
    task_id: Uuid,
    execution_id: &str,
    error_msg: String,
) {
    warn!("{}", error_msg);
    let mut states = process_states.write().await;
    states.insert(execution_id.to_string(), ProcessStatus::Failed);
    drop(states);
    if let Some(tx) = status_tx.read().await.as_ref() {
        let _ = tx.send(StatusUpdate {
            task_id,
            status: "failed".to_string(),
            log: None,
            output: None,
            error: Some(error_msg),
            progress: None,
        });
    }
}

pub(super) async fn send_running_log(
    status_tx: &Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
    task_id: Uuid,
    task_label: &str,
    task_what: &str,
    attempt: Option<&str>,
) {
    if let Some(tx) = status_tx.read().await.as_ref() {
        let attempt_info = attempt
            .map(|a| format!(" (attempt {a})"))
            .unwrap_or_default();
        let _ = tx.send(StatusUpdate {
            task_id,
            status: "running".to_string(),
            log: Some(format!(
                "S running {task_label}: {task_what}{attempt_info}\n"
            )),
            output: None,
            error: None,
            progress: None,
        });
    }
}

pub(super) fn spawn_stdout_log_task(
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
    task_id: Uuid,
    output_store: Arc<Mutex<Option<serde_json::Value>>>,
    stdout: Option<tokio::process::ChildStdout>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout.expect("stdout must be piped"));
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
                *output_store.lock().await = Some(value);
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
            if let Some(json_str) = trimmed.strip_prefix("CZAR_METRIC:") {
                if let Some(tx) = status_tx.read().await.as_ref() {
                    let ts = chrono::Utc::now().format("%H:%M:%S%.3f");
                    let _ = tx.send(StatusUpdate {
                        task_id,
                        status: "log".to_string(),
                        log: Some(format!("M {ts} {}\n", json_str.trim())),
                        output: None,
                        error: None,
                        progress: None,
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
    })
}

pub(super) fn spawn_stderr_log_task(
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
    task_id: Uuid,
    stderr: Option<tokio::process::ChildStderr>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr.expect("stderr must be piped"));
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
    })
}
