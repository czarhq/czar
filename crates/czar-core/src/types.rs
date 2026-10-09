use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::compiled::CompiledWorkflow;
use crate::workflow::{ExecutorKind, Workflow};

pub type RunId = String;
pub type TaskName = String;
pub type WorkflowName = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    #[default]
    Pending,
    Running,
    Paused,
    Success,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Pending,
    Paused,
    Dispatched,
    Running,
    Success,
    Failed,
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Run {
    pub id: RunId,
    pub workflow: WorkflowName,
    pub status: RunStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskRun {
    pub run_id: RunId,
    pub task: TaskName,
    pub status: TaskStatus,
    pub attempt: u32,
    pub max_retries: u32,
    pub created_at: DateTime<Utc>,
    pub dispatched_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub output: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskSpec {
    pub run_id: RunId,
    pub workflow_name: WorkflowName,
    pub task_name: TaskName,
    pub attempt: u32,
    pub executor: ExecutorKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskStatusFile {
    pub status: TaskStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

impl TaskSpec {
    pub fn from_workflow(
        workflow: &Workflow,
        run_id: RunId,
        task_name: &str,
        attempt: u32,
    ) -> Option<Self> {
        let definition = workflow.tasks.get(task_name)?;
        Some(TaskSpec {
            run_id,
            workflow_name: workflow.name.clone(),
            task_name: task_name.to_string(),
            attempt,
            executor: definition.executor.clone(),
        })
    }
}

impl TaskSpec {
    pub fn from_compiled(
        workflow: &CompiledWorkflow,
        run_id: RunId,
        task_idx: usize,
        attempt: u32,
    ) -> Option<Self> {
        let definition = workflow.tasks.get(task_idx)?;
        Some(TaskSpec {
            run_id,
            workflow_name: workflow.name.clone(),
            task_name: definition.name.clone(),
            attempt,
            executor: definition.executor.clone(),
        })
    }
}

impl Run {
    pub fn new(workflow: &Workflow) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            workflow: workflow.name.clone(),
            status: RunStatus::Pending,
            created_at: now,
            started_at: None,
            finished_at: None,
        }
    }
}

#[cfg(test)]
mod tests;
