use std::sync::Arc;
use std::time::Duration;

use czar_core::database::Database;
use czar_core::services::worker_sync::sync_all_workers;
use czar_core::worker_registry::{WorkerConfig, WorkersConfig};

/// Merge workers from the orchestrator config and an optional workers YAML file
/// (or the `CZAR_WORKERS_FILE` env var).  Config-file workers come first.
pub fn load_workers_config(
    workers_file: Option<&str>,
    config_workers: &[WorkerConfig],
) -> Vec<WorkerConfig> {
    let mut workers: Vec<WorkerConfig> = config_workers.to_vec();

    let path = workers_file
        .map(|s| s.to_string())
        .or_else(|| std::env::var("CZAR_WORKERS_FILE").ok());

    if let Some(path) = path {
        match read_workers_from_file(&path) {
            Ok(extra) => workers.extend(extra),
            Err(e) => eprintln!("{e}"),
        }
    }

    for error in resolve_worker_tokens(&mut workers, |name| std::env::var(name).ok()) {
        eprintln!("{error}");
    }
    workers
}

/// Fill in each worker's `token` from its `token_env` variable. Returns one
/// message per worker whose variable is missing or empty, or that sets both.
pub fn resolve_worker_tokens(
    workers: &mut [WorkerConfig],
    env: impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for worker in workers.iter_mut() {
        let Some(name) = worker.token_env.as_deref() else {
            continue;
        };
        if worker.token.is_some() {
            errors.push(format!(
                "Worker {}: set either token or token_env, not both; using token_env",
                worker.url
            ));
        }
        match env(name).filter(|value| !value.trim().is_empty()) {
            Some(value) => worker.token = Some(value),
            None => {
                worker.token = None;
                errors.push(format!(
                    "Worker {}: token_env {name} is not set; its requests go out without a token",
                    worker.url
                ));
            }
        }
    }
    errors
}

/// Read and parse a workers YAML file, returning the list of worker configs.
fn read_workers_from_file(path: &str) -> Result<Vec<WorkerConfig>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read workers file {path}: {e}"))?;
    WorkersConfig::from_yaml(&content)
        .map(|wc| wc.workers)
        .map_err(|e| format!("Failed to parse workers file {path}: {e}"))
}

/// Spawn background tasks that (a) immediately sync all remote workers and
/// (b) start a polling loop for any worker that has `poll_interval_secs` set.
pub fn spawn_worker_sync_tasks<D: Database + 'static>(db: &Arc<D>, workers: &[WorkerConfig]) {
    if workers.is_empty() {
        return;
    }

    let sync_db = db.clone();
    let sync_workers = workers.to_vec();
    tokio::spawn(async move {
        sync_all_workers(&*sync_db, &sync_workers).await;
    });

    for worker in workers {
        if let Some(interval_secs) = worker.poll_interval_secs {
            let poll_db = db.clone();
            let poll_worker = worker.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(interval_secs)).await;
                    czar_core::services::worker_sync::sync_worker(&*poll_db, &poll_worker)
                        .await
                        .unwrap_or_else(|e| {
                            eprintln!("Worker poll failed for {}: {e}", poll_worker.url);
                            czar_core::services::worker_sync::WorkerSyncResult {
                                worker_url: poll_worker.url.clone(),
                                workflow_name: String::new(),
                            }
                        });
                }
            });
        }
    }
}
