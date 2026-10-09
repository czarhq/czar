use std::{
    collections::BTreeMap,
    collections::BTreeSet,
    collections::HashMap,
    path::{Path, PathBuf},
};

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{CzarError, CzarResult, WorkflowValidationError};

mod defaults;
mod flow;
mod runtime_types;
pub use flow::*;
pub use runtime_types::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Workflow {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub schedule: Option<String>,
    #[serde(default)]
    pub types: BTreeMap<String, serde_json::Value>,
    /// Workflow-level variables available for interpolation in `file`, `command`, and `args`
    /// fields using `${{ vars.NAME }}` syntax.
    ///
    /// Variables can be overridden at runtime via environment variables using the
    /// `CZAR_VAR_<NAME>` convention (e.g. `CZAR_VAR_BUILD_PROFILE=release`).
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    pub tasks: IndexMap<String, TaskDefinition>,
    /// Lifecycle hooks inherited from the workflow definition.
    #[serde(default)]
    pub hooks: IndexMap<String, HookDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskDefinition {
    pub executor: ExecutorKind,
    pub image: Option<String>,
    pub file: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub command: Option<String>,
    /// Extra arguments appended to the command (process) or wasmtime invocation (wasm).
    /// For `process`, when `args` is set the command is spawned directly (no `sh -c`)
    /// to avoid shell-quoting issues and injection risks.
    #[serde(default)]
    pub args: Vec<String>,
    /// Execution mode for `wasm` tasks. Defaults to `embedded` (in-process via wasmtime crate).
    /// Set to `cli` for WASI _start modules that use stdout-based I/O.
    #[serde(default)]
    pub wasm_mode: WasmMode,
    pub job: Option<String>,
    pub module: Option<String>,
    pub function: Option<String>,
    #[serde(skip)]
    pub custom_executor: Option<CustomExecutorDefinition>,
    #[serde(default)]
    pub inputs: serde_json::Value,
    #[serde(default)]
    pub env: serde_json::Value,
    #[serde(default)]
    pub secrets: Vec<SecretFileMount>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout: u64,
    #[serde(default = "default_retries")]
    pub retries: u32,
    #[serde(default)]
    pub retry_delay: Option<u64>,
    #[serde(default)]
    pub on_failure: FailurePolicy,
    #[serde(default)]
    pub input_type: Option<serde_json::Value>,
    #[serde(default)]
    pub output_type: Option<serde_json::Value>,
    #[serde(default = "default_true")]
    pub validate_output: bool,
    /// Optional condition expression. The task only executes when this evaluates to `true`.
    /// Uses the Czar condition DSL, e.g.:
    ///   `tasks.check.output.score > 0.8`
    ///   `tasks.validate.output.is_valid == true and tasks.check.output.ready == true`
    /// Referenced tasks must be listed in `depends_on`.
    #[serde(default)]
    pub when: Option<String>,
    /// How many times to run this task in parallel.
    /// - `parallel: 4` — static count (4 instances at compile time)
    /// - `parallel: discover.count` — dynamic count from an upstream task output field
    #[serde(default)]
    pub parallel: Option<ParallelSpec>,
    /// Expand into one instance per item in the given upstream list output.
    /// Example: `foreach: discover.files`
    #[serde(default)]
    pub foreach: Option<String>,
    /// Maximum number of parallel instances running simultaneously.
    /// Only relevant when `parallel` or `foreach` is set.
    #[serde(default)]
    pub concurrency: Option<u32>,
    /// How the parallel instances are executed.
    #[serde(default)]
    pub parallel_mode: ParallelMode,
}

/// Specifies parallelism — either a fixed count or a reference to an upstream output field.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ParallelSpec {
    /// Static count known at compile time (e.g., `parallel: 4`)
    Count(u32),
    /// Dynamic count from upstream task output (e.g., `parallel: discover.count`)
    Ref(String),
}

/// Determines how parallel task instances are launched.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ParallelMode {
    /// Create one Czar task record per instance (default).
    #[default]
    Czar,
    /// Use Cloud Run's native `task_count` to run all instances as a single execution.
    /// Reduces Czar metadata overhead — ideal for large fan-outs with Cloud Run Jobs.
    CloudRunTasks,
}

/// Events that can trigger a hook.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    /// Fires when a task is dispatched / starts executing.
    #[serde(rename = "task.start")]
    #[default]
    TaskStart,
    /// Fires when a task completes successfully.
    #[serde(rename = "task.success")]
    TaskSuccess,
    /// Fires when a task fails (after exhausting all retries).
    #[serde(rename = "task.failed")]
    TaskFailed,
    /// Fires when a task is skipped (condition was false or upstream skipped).
    #[serde(rename = "task.skipped")]
    TaskSkipped,
    /// Fires when a run transitions to Running.
    #[serde(rename = "run.start")]
    RunStart,
    /// Fires when a run completes successfully.
    #[serde(rename = "run.success")]
    RunSuccess,
    /// Fires when a run fails (one or more tasks failed).
    #[serde(rename = "run.failed")]
    RunFailed,
}

impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TaskStart => "task.start",
            Self::TaskSuccess => "task.success",
            Self::TaskFailed => "task.failed",
            Self::TaskSkipped => "task.skipped",
            Self::RunStart => "run.start",
            Self::RunSuccess => "run.success",
            Self::RunFailed => "run.failed",
        }
    }

    pub fn is_task_event(&self) -> bool {
        matches!(
            self,
            Self::TaskStart | Self::TaskSuccess | Self::TaskFailed | Self::TaskSkipped
        )
    }

    pub fn is_run_event(&self) -> bool {
        matches!(self, Self::RunStart | Self::RunSuccess | Self::RunFailed)
    }
}

/// A hook definition: code that runs in response to a lifecycle event.
///
/// Hooks share the same executor options as regular tasks but are triggered
/// by events rather than being part of the DAG.
///
/// # Example YAML
/// ```yaml
/// hooks:
///   alert_on_failure:
///     on: task.failed
///     tasks: "*"        # optional: "*" or a list of task names
///     executor: python
///     module: alerts
///     function: notify_failure
/// ```
///
/// The hook executor receives a `task_input` with context:
/// - `event`: the event name (e.g. `"task.failed"`)
/// - `hook_name`: the name of this hook definition
/// - `run_id`: UUID of the current run
/// - `workflow_name`: name of the workflow
/// - For task events: `task_name`, `error` (nullable), `output` (nullable)
/// - For run.failed: `failed_tasks` list
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HookDefinition {
    /// The event that triggers this hook.
    pub on: HookEvent,
    /// Optional filter: which task names to watch (only for task-level events).
    /// Use `"*"` or omit for all tasks; otherwise provide a list of task names.
    #[serde(default)]
    pub tasks: Option<HookTaskFilter>,
    /// Executor kind (same options as regular tasks).
    pub executor: ExecutorKind,
    pub file: Option<std::path::PathBuf>,
    pub command: Option<String>,
    pub job: Option<String>,
    pub module: Option<String>,
    pub function: Option<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout: u64,
}

/// Which tasks a hook should fire for (only relevant for task-level events).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum HookTaskFilter {
    /// A wildcard — fires for every task.
    Star(String), // must be "*"
    /// Fires only for the listed tasks.
    List(Vec<String>),
}

impl HookTaskFilter {
    /// Returns `true` if this filter includes the given task name.
    pub fn matches(&self, task_name: &str) -> bool {
        match self {
            Self::Star(s) => s == "*",
            Self::List(names) => names.iter().any(|n| n == task_name),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowDefinition {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub schedule: Option<String>,
    #[serde(default)]
    pub types: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    #[serde(default)]
    #[serde(alias = "import")]
    pub imports: Vec<DbtImportDefinition>,
    #[serde(default)]
    pub task_groups: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub tasks: IndexMap<String, TaskTemplateDefinition>,
    #[serde(default)]
    pub flows: IndexMap<String, FlowDefinition>,
    /// Lifecycle hooks — code that runs in response to task/run events.
    #[serde(default)]
    pub hooks: IndexMap<String, HookDefinition>,
    #[serde(skip)]
    pub executor_registry: BTreeMap<String, CustomExecutorDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskTemplateDefinition {
    #[serde(default)]
    pub executor: Option<ExecutorKind>,
    pub image: Option<String>,
    pub file: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub wasm_mode: WasmMode,
    pub job: Option<String>,
    pub module: Option<String>,
    pub function: Option<String>,
    #[serde(skip)]
    pub custom_executor: Option<CustomExecutorDefinition>,
    #[serde(default)]
    pub inputs: serde_json::Value,
    #[serde(default)]
    pub env: serde_json::Value,
    #[serde(default)]
    pub secrets: Vec<SecretFileMount>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout: u64,
    #[serde(default = "default_retries")]
    pub retries: u32,
    #[serde(default)]
    pub retry_delay: Option<u64>,
    #[serde(default)]
    pub on_failure: FailurePolicy,
    #[serde(default)]
    pub input_type: Option<serde_json::Value>,
    #[serde(default)]
    pub output_type: Option<serde_json::Value>,
    #[serde(default = "default_true")]
    pub validate_output: bool,
    /// Optional condition expression (see `TaskDefinition::when`).
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub parallel: Option<ParallelSpec>,
    #[serde(default)]
    pub foreach: Option<String>,
    #[serde(default)]
    pub concurrency: Option<u32>,
    #[serde(default)]
    pub parallel_mode: ParallelMode,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct SecretFileMount {
    pub name: String,
    pub secret: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DbtImportDefinition {
    pub dbt: PathBuf,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub select: Vec<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default = "default_true")]
    pub include_models: bool,
    #[serde(default = "default_true")]
    pub include_tests: bool,
    #[serde(default = "default_true")]
    pub include_seeds: bool,
    #[serde(default = "default_true")]
    pub include_snapshots: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub profiles_dir: Option<PathBuf>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub threads: Option<u32>,
}

fn default_timeout_secs() -> u64 {
    300
}

fn default_retries() -> u32 {
    0
}

fn default_true() -> bool {
    true
}

const ALL_TASKS_FLOW: &str = "__all__";

mod workflow_definition_build;
mod workflow_definition_catalog;
mod workflow_definition_impl;
mod workflow_imports;
pub(crate) mod workflow_validation_helpers;
mod workflow_validation_impl;

#[cfg(test)]
mod workflow_tests_core;
#[cfg(test)]
mod workflow_tests_definition;
#[cfg(test)]
mod workflow_tests_types;
