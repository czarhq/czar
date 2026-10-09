use anyhow::{Result, anyhow};
use std::path::Path;

use crate::database::Database;
use crate::services::workflow_yaml::{ApplyWorkflowYamlRequest, apply_workflow_yaml};
use crate::worker_client::WorkerClient;
use crate::worker_registry::{WorkerConfig, WorkerRegistrationMetadata, with_worker_metadata};
use crate::workflow::ValidationOptions;

pub struct WorkerSyncResult {
    pub worker_url: String,
    pub workflow_name: String,
}

/// Sync all workers from the config, logging errors per-worker without aborting.
pub async fn sync_all_workers(db: &dyn Database, workers: &[WorkerConfig]) {
    for worker in workers {
        let label = worker
            .name
            .as_deref()
            .unwrap_or(worker.url.as_str())
            .to_string();
        match sync_worker(db, worker).await {
            Ok(r) => {
                println!(
                    "Synced workflow '{}' from worker {} ({})",
                    r.workflow_name, label, r.worker_url
                );
            }
            Err(e) => {
                eprintln!("Failed to sync worker {} ({}): {}", label, worker.url, e);
            }
        }
    }
}

/// Call `/compile` on a worker and register the resulting workflow in the DB.
///
/// The workflow's `task_params` is stamped with the worker URL so the executor
/// manager can route tasks back to this worker at execution time.
pub async fn sync_worker(db: &dyn Database, config: &WorkerConfig) -> Result<WorkerSyncResult> {
    let client = WorkerClient::new(config.url.clone()).with_token(config.token.clone());

    let compiled = client
        .compile(None)
        .await
        .map_err(|e| anyhow!("failed to compile from worker {}: {}", config.url, e))?;

    let source_yaml = compiled.source_yaml.ok_or_else(|| {
        anyhow!(
            "worker {} compile response did not include source_yaml",
            config.url
        )
    })?;

    let workflow_name = compiled
        .workflow
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow!(
                "worker {} compile response missing workflow name",
                config.url
            )
        })?
        .to_string();

    let meta = WorkerRegistrationMetadata {
        worker_url: config.url.clone(),
    };

    // Workers may use executor types the orchestrator doesn't have locally,
    // so skip strict executor-type validation.
    let request = ApplyWorkflowYamlRequest::new(&source_yaml, Path::new("."), "", "")
        .with_validation_options(ValidationOptions::RELAXED);

    let result = apply_workflow_yaml(db, request)
        .await
        .map_err(|e| anyhow!("failed to apply workflow from worker {}: {}", config.url, e))?;

    // Stamp the worker URL into task_params after the workflow is stored.
    let existing_params = result
        .workflow
        .task_params
        .as_ref()
        .map(crate::models::json_inner)
        .cloned();
    let new_params = with_worker_metadata(existing_params, &meta);
    db.update_workflow_task_params(result.workflow.id, Some(new_params))
        .await
        .map_err(|e| {
            anyhow!(
                "failed to persist worker metadata for workflow {}: {}",
                workflow_name,
                e
            )
        })?;

    Ok(WorkerSyncResult {
        worker_url: config.url.clone(),
        workflow_name,
    })
}
