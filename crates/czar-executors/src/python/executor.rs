use anyhow::Result;
use async_trait::async_trait;
use czar_core::executor::{Executor, StatusUpdate};
use czar_core::resolve_python_execution_root;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::process::ProcessExecutor;

pub struct PythonExecutor {
    process: ProcessExecutor,
    workflow_root: Option<PathBuf>,
}

impl PythonExecutor {
    pub fn new(working_dir: Option<String>, workflow_root: Option<PathBuf>) -> Self {
        Self {
            process: ProcessExecutor::new(working_dir),
            workflow_root,
        }
    }
}

#[async_trait]
impl Executor for PythonExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        job_name: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        let params = resolve_python_params(params, self.workflow_root.as_deref(), job_name)?;
        self.process.execute(task_id, job_name, Some(params)).await
    }

    async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>) {
        self.process.set_status_channel(tx).await;
    }
}

pub fn resolve_python_params(
    params: Option<serde_json::Value>,
    workflow_root: Option<&Path>,
    task_name: &str,
) -> Result<serde_json::Value> {
    let mut params = params.unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let obj = params
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Python executor requires object params"))?;

    let task_file = obj
        .get("task_file")
        .and_then(|v| v.as_str())
        .map(|s| resolve_relative_path(s, workflow_root));

    if let Some(ref tf) = task_file {
        obj.insert(
            "task_file".to_string(),
            serde_json::Value::String(tf.to_string_lossy().to_string()),
        );
    }

    let explicit_root = obj
        .get("python_path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);

    let python_path = resolve_python_execution_root(
        explicit_root.as_deref(),
        task_file.as_deref(),
        workflow_root,
        &format!("Python task {task_name}"),
    )
    .map_err(|e| anyhow::anyhow!(e))?;

    obj.insert(
        "python_path".to_string(),
        serde_json::Value::String(python_path.to_string_lossy().to_string()),
    );

    Ok(params)
}

fn resolve_relative_path(path: &str, workflow_root: Option<&Path>) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else if let Some(root) = workflow_root {
        root.join(p)
    } else {
        p
    }
}
