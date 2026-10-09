use crate::models::{Workflow, json_inner};
use serde::{Deserialize, Serialize};

const WORKER_KEY: &str = "worker";
const WORKER_URL_KEY: &str = "url";

/// Config for a single remote worker, as written in the YAML config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Base URL of the worker HTTP server, e.g. "http://worker:8081"
    pub url: String,

    /// Optional display name; defaults to the URL.
    #[serde(default)]
    pub name: Option<String>,

    /// Re-sync interval in seconds. If absent, only synced on startup / manual trigger.
    #[serde(default)]
    pub poll_interval_secs: Option<u64>,

    /// Bearer token the worker was started with (`czar-worker --auth-token`).
    /// Prefer `token_env`, so the token isn't written into the config file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Name of an environment variable holding the token, e.g.
    /// `CZAR_WORKER_SALES_TOKEN`. Read when `czar serve` loads its workers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
}

/// Top-level workers config file (`workers.yaml`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkersConfig {
    pub workers: Vec<WorkerConfig>,
}

impl WorkersConfig {
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }
}

// ---------------------------------------------------------------------------
// Per-workflow metadata — stored in workflow.task_params under "worker" key
// ---------------------------------------------------------------------------

/// Stored in a workflow's `task_params` when the workflow was sourced from a
/// remote worker.  The executor manager reads this to route tasks back to the
/// correct worker instead of dispatching locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRegistrationMetadata {
    pub worker_url: String,
}

pub fn with_worker_metadata(
    task_params: Option<serde_json::Value>,
    meta: &WorkerRegistrationMetadata,
) -> serde_json::Value {
    let mut params = task_params.unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = params.as_object_mut() {
        obj.insert(
            WORKER_KEY.to_string(),
            serde_json::json!({ WORKER_URL_KEY: meta.worker_url }),
        );
    }
    params
}

pub fn worker_metadata_from_workflow(workflow: &Workflow) -> Option<WorkerRegistrationMetadata> {
    let params = workflow.task_params.as_ref().map(json_inner)?;
    worker_metadata_from_params(params)
}

fn worker_metadata_from_params(params: &serde_json::Value) -> Option<WorkerRegistrationMetadata> {
    let worker = params.get(WORKER_KEY)?;
    let url = worker.get(WORKER_URL_KEY)?.as_str()?.to_string();
    Some(WorkerRegistrationMetadata { worker_url: url })
}

#[cfg(test)]
mod tests;
