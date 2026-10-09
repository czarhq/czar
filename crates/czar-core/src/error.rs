use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CzarError {
    #[error("failed to read file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write file {path}: {source}")]
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("yaml parse error in {path}: {source}")]
    YamlParse {
        path: PathBuf,
        source: serde_yaml::Error,
    },

    #[error("json parse error in {path}: {source}")]
    JsonParse {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("invalid workflow: {0}")]
    InvalidWorkflow(WorkflowValidationError),

    #[error("executor error for task {task}: {source}")]
    ExecutorError {
        task: String,
        source: std::io::Error,
    },

    #[error("task {task} timed out after {seconds}s")]
    Timeout { task: String, seconds: u64 },
}

pub type CzarResult<T> = Result<T, CzarError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkflowValidationError {
    #[error("workflow name cannot be empty")]
    EmptyWorkflowName,
    #[error("workflow must contain at least one task")]
    NoTasks,
    #[error("task name cannot be empty")]
    EmptyTaskName,
    #[error("task {task} depends on unknown task {dependency}")]
    UnknownDependency { task: String, dependency: String },
    #[error("task {task} cannot depend on itself")]
    SelfDependency { task: String },
    #[error("task {task} requires a `file` path or `module` + `function`")]
    MissingTaskFile { task: String },
    #[error("task {task} requires a `command` or `file` path")]
    MissingTaskCommand { task: String },
    #[error("task {task} requires an `image`")]
    MissingTaskImage { task: String },
    #[error("task {task} requires a `job` name")]
    MissingTaskJob { task: String },
    #[error(
        "task {task} file not found: declared '{declared_path}' resolved relative to workflow directory '{workflow_root}' -> '{resolved_path}'"
    )]
    TaskFileNotFound {
        task: String,
        declared_path: PathBuf,
        resolved_path: PathBuf,
        workflow_root: PathBuf,
    },
    #[error("workflow contains a cycle involving task {task}")]
    Cycle { task: String },
    #[error("task '{task}' has invalid type schema at '{path}': {reason}")]
    InvalidTypeSchema {
        task: String,
        path: String,
        reason: String,
    },
    #[error("{0}")]
    Custom(String),
}
