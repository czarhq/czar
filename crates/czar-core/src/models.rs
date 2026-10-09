// Database-backed orchestration models with event-driven architecture support
// These models include rich metadata for orchestration (timestamps, execution tracking, etc.)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "sqlx")]
use sqlx::types::Json;

// Type alias for JSON fields - when sqlx is enabled use sqlx::Json, otherwise use direct Value
#[cfg(feature = "sqlx")]
pub type JsonValue = Json<serde_json::Value>;

#[cfg(not(feature = "sqlx"))]
pub type JsonValue = serde_json::Value;

// Helper to construct JSON fields uniformly across sqlx and non-sqlx builds.
#[cfg(feature = "sqlx")]
#[inline]
pub fn json_value(value: serde_json::Value) -> JsonValue {
    Json(value)
}

#[cfg(not(feature = "sqlx"))]
#[inline]
pub fn json_value(value: serde_json::Value) -> JsonValue {
    value
}

// Helper to access inner JSON value uniformly
#[cfg(feature = "sqlx")]
#[inline]
pub fn json_inner(j: &JsonValue) -> &serde_json::Value {
    &j.0
}

#[cfg(not(feature = "sqlx"))]
#[inline]
pub fn json_inner(j: &JsonValue) -> &serde_json::Value {
    j
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Workflow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub job_name: String,
    pub region: String,
    pub project: String,
    pub executor_type: String,
    pub task_params: Option<JsonValue>,
    pub schedule: Option<String>,
    pub schedule_enabled: bool,
    pub last_scheduled_at: Option<DateTime<Utc>>,
    pub next_scheduled_at: Option<DateTime<Utc>>,
    pub current_snapshot_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct WorkflowSnapshot {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub content_hash: String,
    pub tasks_json: JsonValue,
    pub source_yaml: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutorType {
    #[default]
    Container,
    CloudRun,
    Process,
    Python,
    Library,
    Wasm,
}

impl ExecutorType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "container" => Some(Self::Container),
            "cloudrun" => Some(Self::CloudRun),
            "process" => Some(Self::Process),
            "python" => Some(Self::Python),
            "library" => Some(Self::Library),
            "wasm" => Some(Self::Wasm),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::CloudRun => "cloudrun",
            Self::Process => "process",
            Self::Python => "python",
            Self::Library => "library",
            Self::Wasm => "wasm",
        }
    }
}

impl std::fmt::Display for ExecutorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ExecutorType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "TEXT", rename_all = "lowercase"))]
pub enum TaskStatus {
    #[default]
    Pending,
    Dispatched,
    Running,
    Paused,
    Success,
    Failed,
    Cancelled,
    /// Task was skipped because its `when` condition evaluated to false.
    /// Downstream tasks that depend on a skipped task are also skipped.
    Skipped,
}

impl TaskStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "dispatched" => Some(Self::Dispatched),
            "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "skipped" => Some(Self::Skipped),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Dispatched => "dispatched",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Success | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "TEXT", rename_all = "lowercase"))]
pub enum RunStatus {
    #[default]
    Pending,
    Running,
    Paused,
    Success,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Success | Self::Failed | Self::Cancelled)
    }
}

impl std::str::FromStr for RunStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskSelectionMode {
    #[default]
    Exact,
    Upstream,
    Downstream,
    Full,
}

impl TaskSelectionMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "exact" => Some(Self::Exact),
            "upstream" => Some(Self::Upstream),
            "downstream" => Some(Self::Downstream),
            "full" => Some(Self::Full),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RunTaskSelection {
    pub mode: TaskSelectionMode,
    #[serde(default)]
    pub flow_name: Option<String>,
    #[serde(default)]
    pub requested_tasks: Vec<String>,
    #[serde(default)]
    pub requested_groups: Vec<String>,
    #[serde(default)]
    pub resolved_tasks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RunLaunchConfig {
    #[serde(default)]
    pub task_selection: Option<RunTaskSelection>,
    #[serde(default)]
    pub inputs: Option<serde_json::Value>,
}

impl RunLaunchConfig {
    pub fn parse(value: Option<&JsonValue>) -> anyhow::Result<Self> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let raw = json_inner(value).clone();
        if raw.is_null() {
            return Ok(Self::default());
        }

        if let Ok(selection) = serde_json::from_value::<RunTaskSelection>(raw.clone()) {
            return Ok(Self {
                task_selection: Some(selection),
                inputs: None,
            });
        }

        let config: Self = serde_json::from_value(raw)?;
        config.validate()?;
        Ok(config)
    }

    pub fn to_storage_value(&self) -> anyhow::Result<Option<serde_json::Value>> {
        self.validate()?;
        if self.task_selection.is_none() && self.inputs.is_none() {
            return Ok(None);
        }
        if self.inputs.is_none() {
            return self
                .task_selection
                .as_ref()
                .map(|selection| serde_json::to_value(selection).map_err(anyhow::Error::from))
                .transpose();
        }
        serde_json::to_value(self)
            .map(Some)
            .map_err(anyhow::Error::from)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if let Some(inputs) = self.inputs.as_ref()
            && !inputs.is_object()
        {
            return Err(anyhow::anyhow!(
                "run launch inputs must be an object keyed by input name"
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Run {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub snapshot_id: Option<Uuid>,
    #[serde(default)]
    pub task_selection: Option<JsonValue>,
    pub status: RunStatus,
    pub triggered_by: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Run {
    pub fn status(&self) -> RunStatus {
        self.status
    }

    pub fn status_str(&self) -> &str {
        self.status.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Task {
    pub id: Uuid,
    pub run_id: Uuid,
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: Vec<String>,
    pub status: TaskStatus,
    #[serde(default)]
    pub attempts: i32,
    #[serde(default)]
    pub max_retries: i32,
    #[serde(default)]
    pub timeout_seconds: Option<i32>,
    #[serde(default)]
    pub retry_at: Option<DateTime<Utc>>,
    pub execution_name: Option<String>,
    pub params: Option<JsonValue>,
    pub output: Option<JsonValue>,
    pub logs: Option<String>,
    pub error: Option<String>,
    /// Live in-task progress. Shape: `{"current": u64, "total": u64, "message": str}`.
    /// Written by the scheduler when the task emits `CZAR_PROGRESS:` lines.
    #[serde(default)]
    pub progress: Option<JsonValue>,
    pub dispatched_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Task {
    pub fn status(&self) -> TaskStatus {
        self.status
    }

    pub fn status_str(&self) -> &str {
        self.status.as_str()
    }
}

/// Combined task and workflow data to avoid N+1 queries
/// Used by scheduler to fetch tasks with their workflow info in a single query
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct TaskWithWorkflow {
    // Task fields
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: Vec<String>,
    pub task_status: TaskStatus,
    pub attempts: i32,
    pub max_retries: i32,
    pub timeout_seconds: Option<i32>,
    pub retry_at: Option<DateTime<Utc>>,
    pub execution_name: Option<String>,
    pub params: Option<JsonValue>,

    // Workflow fields
    pub workflow_id: Uuid,
    pub job_name: String,
    pub project: String,
    pub region: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct WorkflowTask {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: Vec<String>,
    pub params: Option<JsonValue>,
    pub signature: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct WorkflowTaskState {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub task_name: String,
    pub last_status: TaskStatus,
    pub last_run_id: Uuid,
    pub last_task_id: Uuid,
    pub last_output: Option<JsonValue>,
    pub last_error: Option<String>,
    pub last_finished_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "TEXT", rename_all = "lowercase"))]
pub enum DeferredJobStatus {
    #[default]
    Pending,
    Polling,
    Completed,
    Failed,
    Cancelled,
}

impl DeferredJobStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "polling" => Some(Self::Polling),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Polling => "polling",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for DeferredJobStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

/// Represents a long-running external job being tracked by the scheduler
/// Used by the Triggerer component to poll external APIs (BigQuery, Cloud Run, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct DeferredJob {
    pub id: Uuid,
    pub task_id: Uuid,
    pub service_type: String,
    pub job_id: String,
    pub job_data: JsonValue,
    pub status: DeferredJobStatus,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_polled_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl DeferredJob {
    pub fn status(&self) -> DeferredJobStatus {
        self.status
    }

    pub fn status_str(&self) -> &str {
        self.status.as_str()
    }
}

#[cfg(test)]
mod tests;
