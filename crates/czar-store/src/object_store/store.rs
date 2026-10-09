use std::path::PathBuf;

use async_trait::async_trait;

use czar_core::error::CzarResult;
use czar_core::types::{TaskSpec, TaskStatusFile};

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn write_spec(&self, spec: &TaskSpec) -> CzarResult<PathBuf>;
    async fn read_spec(&self, run_id: &str, task_name: &str) -> CzarResult<TaskSpec>;
    fn spec_path(&self, run_id: &str, task_name: &str) -> PathBuf;
    fn task_dir(&self, run_id: &str, task_name: &str) -> PathBuf;
    fn output_path(&self, run_id: &str, task_name: &str) -> PathBuf;

    async fn write_status(
        &self,
        run_id: &str,
        task_name: &str,
        status: &TaskStatusFile,
    ) -> CzarResult<()>;
    async fn read_status(
        &self,
        run_id: &str,
        task_name: &str,
    ) -> CzarResult<Option<TaskStatusFile>>;

    async fn write_output(
        &self,
        run_id: &str,
        task_name: &str,
        output: &serde_json::Value,
    ) -> CzarResult<()>;
    async fn read_output(
        &self,
        run_id: &str,
        task_name: &str,
    ) -> CzarResult<Option<serde_json::Value>>;
}
