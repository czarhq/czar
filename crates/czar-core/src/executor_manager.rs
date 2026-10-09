// ExecutorManager trait for managing executor instances
// Concrete implementation in czar-executors crate

use crate::executor::Executor;
use crate::models::Workflow;
use async_trait::async_trait;
use std::sync::Arc;

/// ExecutorManager trait for managing workflow executors
///
/// This trait abstracts the management of executor instances.
/// Different implementations can handle different executor backends.
#[async_trait]
pub trait ExecutorManager: Send + Sync {
    /// Get or create an executor for a task.
    async fn get_executor(
        &self,
        executor_type: &str,
        workflow: &Workflow,
    ) -> anyhow::Result<Arc<dyn Executor>>;
}
