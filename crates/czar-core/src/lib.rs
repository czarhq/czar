pub mod condition;
pub mod error;
pub mod types;
pub mod workflow;

pub mod compiled;

// Database-backed architecture (event-driven, high-performance)
pub mod config;
pub mod database;
pub mod events;
pub mod executor;
pub mod models;
pub mod notifications;
pub mod schedule_processor;
pub mod services;
pub mod task_execution;

pub use error::{CzarError, CzarResult};
pub use types::*;
pub use workflow::{
    ExecutorKind, HookDefinition, HookEvent, HookTaskFilter, TaskDefinition, Workflow,
    WorkflowDefinition,
};

pub mod executor_manager;
pub mod hooks;
pub mod job_tracker;
pub mod scheduler;
pub mod triggerer;
pub mod worker_client;
pub mod worker_registry;
pub mod workflow_registry;
pub mod workflow_source;

pub(crate) mod python_runtime;
pub use python_runtime::resolve_python_execution_root;
