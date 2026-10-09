use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

#[derive(Clone)]
pub struct FileDatabase {
    pub(super) base: PathBuf,
}

impl Default for FileDatabase {
    fn default() -> Self {
        Self::new(".czar/db")
    }
}

impl FileDatabase {
    pub fn new(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref().to_path_buf();
        Self { base }
    }

    pub(super) fn workflows_dir(&self) -> PathBuf {
        self.base.join("workflows")
    }

    pub(super) fn workflow_path(&self, id: Uuid) -> PathBuf {
        self.workflows_dir().join(format!("{}.json", id))
    }

    pub(super) fn runs_dir(&self) -> PathBuf {
        self.base.join("runs")
    }

    pub(super) fn run_path(&self, id: Uuid) -> PathBuf {
        self.runs_dir().join(format!("{}.json", id))
    }

    pub(super) fn tasks_dir(&self) -> PathBuf {
        self.base.join("tasks")
    }

    pub(super) fn task_path(&self, id: Uuid) -> PathBuf {
        self.tasks_dir().join(format!("{}.json", id))
    }

    pub(super) fn workflow_tasks_dir(&self) -> PathBuf {
        self.base.join("workflow_tasks")
    }

    pub(super) fn workflow_tasks_path(&self, workflow_id: Uuid) -> PathBuf {
        self.workflow_tasks_dir()
            .join(format!("{}.json", workflow_id))
    }

    pub(super) fn workflow_task_states_dir(&self) -> PathBuf {
        self.base.join("workflow_task_states")
    }

    pub(super) fn workflow_task_states_path(&self, workflow_id: Uuid) -> PathBuf {
        self.workflow_task_states_dir()
            .join(format!("{}.json", workflow_id))
    }

    pub(super) async fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.workflows_dir()).await?;
        fs::create_dir_all(self.runs_dir()).await?;
        fs::create_dir_all(self.tasks_dir()).await?;
        fs::create_dir_all(self.workflow_tasks_dir()).await?;
        fs::create_dir_all(self.workflow_task_states_dir()).await?;
        Ok(())
    }

    pub async fn run_migrations(&self) -> Result<()> {
        self.ensure_dirs().await
    }

    pub(super) async fn write_json<T: Serialize>(&self, path: &Path, data: &T) -> Result<()> {
        let json = serde_json::to_string_pretty(data)?;
        fs::write(path, json).await?;
        Ok(())
    }

    pub(super) async fn read_json<T: for<'de> Deserialize<'de>>(&self, path: &Path) -> Result<T> {
        let contents = fs::read_to_string(path).await?;
        Ok(serde_json::from_str(&contents)?)
    }
}
