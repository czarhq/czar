// Event-driven executor trait with channel-based status updates
// This replaces polling with push-based notifications for better performance

pub use crate::events::StatusUpdate;

use async_trait::async_trait;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Executor trait for running tasks
/// Executors are responsible for:
/// 1. Dispatching tasks to workers (local processes, Cloud Run, Fargate, etc.)
/// 2. Tracking execution state
/// 3. Pushing status updates via channels (event-driven, not polling)
#[async_trait]
pub trait Executor: Send + Sync {
    /// Execute a task and return the execution identifier
    ///
    /// # Arguments
    /// * `task_id` - UUID of the task being executed
    /// * `job_name` - Name of the job/script to run
    /// * `params` - Optional parameters to pass to the job
    ///
    /// # Returns
    /// Execution identifier (process ID, Cloud Run execution name, etc.)
    async fn execute(
        &self,
        task_id: Uuid,
        job_name: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<String>;

    /// Set the status update channel for event-driven notifications
    /// When a task's status changes, the executor sends a StatusUpdate via this channel
    /// The scheduler receives these updates and batch-processes them
    async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>);
}
