// Database trait for orchestration state
// Implementations: PostgresDatabase, SqliteDatabase

pub mod tracing;

use uuid::Uuid;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::models::{
    DeferredJob, DeferredJobStatus, Run, RunStatus, Task, TaskStatus, TaskWithWorkflow, Workflow,
    WorkflowTaskState,
};

#[derive(Debug, Clone, Default)]
pub struct NewTask {
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: Vec<String>,
    pub params: serde_json::Value,
    pub max_retries: i32,
    pub timeout_seconds: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct NewWorkflowTask {
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: Vec<String>,
    pub params: serde_json::Value,
    pub signature: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowListQuery {
    pub limit: usize,
    pub offset: usize,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowListPage {
    pub items: Vec<Workflow>,
    pub total: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RunListQuery {
    pub limit: usize,
    pub offset: usize,
    pub search: Option<String>,
    pub statuses: Vec<RunStatus>,
    pub workflow_names: Vec<String>,
    pub since: Option<chrono::DateTime<chrono::Utc>>,
    pub until: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct RunListEntry {
    pub run: Run,
    pub workflow_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RunListPage {
    pub items: Vec<RunListEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskRetryMeta {
    pub attempts: i32,
    pub max_retries: i32,
    pub retry_delay_seconds: Option<i32>,
}

/// A single entry for batch-upserting workflow task states.
#[derive(Debug, Clone)]
pub struct WorkflowTaskStateEntry {
    pub workflow_id: Uuid,
    pub task_name: String,
    pub last_status: crate::models::TaskStatus,
    pub last_run_id: Uuid,
    pub last_task_id: Uuid,
    pub last_output: Option<serde_json::Value>,
    pub last_error: Option<String>,
    pub last_finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Repository for workflow CRUD operations
#[async_trait]
pub trait WorkflowRepository: Send + Sync {
    async fn create_workflow(
        &self,
        name: &str,
        description: Option<&str>,
        job_name: &str,
        region: &str,
        project: &str,
        executor_type: &str,
        task_params: Option<serde_json::Value>,
        schedule: Option<&str>,
    ) -> anyhow::Result<Workflow>;

    async fn get_workflow(&self, name: &str) -> anyhow::Result<Workflow>;
    async fn get_workflow_by_id(&self, workflow_id: Uuid) -> anyhow::Result<Workflow>;
    async fn list_workflows(&self) -> anyhow::Result<Vec<Workflow>>;
    async fn list_workflows_page(
        &self,
        query: &WorkflowListQuery,
    ) -> anyhow::Result<WorkflowListPage>;
    async fn delete_workflow(&self, name: &str) -> anyhow::Result<()>;

    /// Batch fetch workflows by IDs (used by scheduler)
    async fn get_workflows_by_ids(&self, workflow_ids: &[Uuid]) -> anyhow::Result<Vec<Workflow>>;

    async fn create_workflow_tasks(
        &self,
        workflow_id: Uuid,
        tasks: &[NewWorkflowTask],
    ) -> anyhow::Result<()>;

    async fn update_workflow_task_params(
        &self,
        workflow_id: Uuid,
        task_params: Option<serde_json::Value>,
    ) -> anyhow::Result<()>;

    async fn list_workflow_tasks(
        &self,
        workflow_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::WorkflowTask>>;
}

/// Repository for workflow snapshot operations (versioning)
#[async_trait]
pub trait WorkflowSnapshotRepository: Send + Sync {
    /// Create a new workflow snapshot or return existing one with same hash
    async fn create_or_get_snapshot(
        &self,
        workflow_id: Uuid,
        content_hash: &str,
        tasks_json: serde_json::Value,
        source_yaml: Option<String>,
    ) -> anyhow::Result<crate::models::WorkflowSnapshot>;

    /// Get a snapshot by ID
    async fn get_snapshot(
        &self,
        snapshot_id: Uuid,
    ) -> anyhow::Result<crate::models::WorkflowSnapshot>;

    /// Update workflow's current_snapshot_id
    async fn update_workflow_snapshot(
        &self,
        workflow_id: Uuid,
        snapshot_id: Uuid,
    ) -> anyhow::Result<()>;
}

/// Repository for run lifecycle operations
#[async_trait]
pub trait RunRepository: Send + Sync {
    async fn create_run(&self, workflow_id: Uuid, triggered_by: &str) -> anyhow::Result<Run>;
    async fn create_run_with_snapshot_and_selection(
        &self,
        workflow_id: Uuid,
        snapshot_id: Option<Uuid>,
        triggered_by: &str,
        task_selection: Option<serde_json::Value>,
    ) -> anyhow::Result<Run>;
    async fn create_run_with_selection(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
        task_selection: Option<serde_json::Value>,
    ) -> anyhow::Result<Run>;

    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: RunStatus,
        error: Option<&str>,
    ) -> anyhow::Result<()>;

    async fn get_run(&self, run_id: Uuid) -> anyhow::Result<Run>;
    async fn list_runs(&self, workflow_id: Option<Uuid>) -> anyhow::Result<Vec<Run>>;
    async fn list_runs_page(&self, query: &RunListQuery) -> anyhow::Result<RunListPage>;
    async fn get_pending_runs(&self) -> anyhow::Result<Vec<Run>>;

    /// Cancel a run and all its pending/dispatched/running tasks
    async fn cancel_run(&self, run_id: Uuid) -> anyhow::Result<()>;

    /// Get run completion stats (total, completed, failed counts)
    /// Used by scheduler to check if a run is complete
    async fn get_run_task_stats(&self, run_id: Uuid) -> anyhow::Result<(i64, i64, i64)>;

    /// Batch version of get_run_task_stats — returns a map of run_id → (total, completed, failed).
    /// Missing run_ids (no tasks yet) are omitted from the result.
    async fn batch_get_run_task_stats(
        &self,
        run_ids: &[Uuid],
    ) -> anyhow::Result<HashMap<Uuid, (i64, i64, i64)>> {
        let mut result = HashMap::new();
        for &run_id in run_ids {
            result.insert(run_id, self.get_run_task_stats(run_id).await?);
        }
        Ok(result)
    }

    /// Batch update run statuses. Each entry is (run_id, status, error).
    async fn batch_update_run_statuses(
        &self,
        updates: &[(Uuid, RunStatus, Option<String>)],
    ) -> anyhow::Result<()> {
        for (run_id, status, error) in updates {
            self.update_run_status(*run_id, *status, error.as_deref())
                .await?;
        }
        Ok(())
    }
}

/// Repository for task lifecycle operations
#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn batch_create_tasks(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> anyhow::Result<()>;

    async fn batch_create_dag_tasks(&self, run_id: Uuid, tasks: &[NewTask]) -> anyhow::Result<()>;

    /// Create DAG tasks for multiple runs in a single round-trip.
    /// Each entry is (run_id, tasks). Default loops; Postgres overrides with one INSERT.
    async fn batch_create_dag_tasks_multi(
        &self,
        runs: &[(Uuid, Vec<NewTask>)],
    ) -> anyhow::Result<()> {
        for (run_id, tasks) in runs {
            self.batch_create_dag_tasks(*run_id, tasks).await?;
        }
        Ok(())
    }

    /// Replace a pending task with one or more materialized runtime tasks.
    ///
    /// This is used for runtime fan-out:
    /// - `parallel: <upstream field>` in `parallel_mode: czar` rewrites one pending
    ///   placeholder task into N concrete tasks such as `task[0]`, `task[1]`, ...
    /// - `parallel: <upstream field>` in `parallel_mode: cloud_run_tasks` rewrites the
    ///   placeholder task in-place with resolved `task_count`.
    ///
    /// Any downstream `depends_on` entries referencing `original_task_name` in the same run
    /// are rewritten to depend on the replacement task names.
    async fn replace_pending_task(
        &self,
        task_id: Uuid,
        original_task_name: &str,
        replacement_tasks: &[NewTask],
    ) -> anyhow::Result<()>;

    async fn update_task_status(
        &self,
        task_id: Uuid,
        status: TaskStatus,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Batch update task statuses (optimized for bulk operations)
    ///
    /// Format: Vec<(task_id, status, execution_name, error)>
    async fn batch_update_task_status(
        &self,
        updates: &[(Uuid, TaskStatus, Option<&str>, Option<&str>)],
    ) -> anyhow::Result<()>;

    async fn list_tasks(&self, run_id: Uuid) -> anyhow::Result<Vec<Task>>;
    async fn get_pending_tasks(&self) -> anyhow::Result<Vec<Task>>;
    async fn get_running_tasks(&self) -> anyhow::Result<Vec<Task>>;
    /// Append log text for multiple tasks in a single round-trip.
    async fn batch_append_task_logs(&self, entries: &[(Uuid, String)]) -> anyhow::Result<()>;

    /// Convenience wrapper: appends a single log chunk.
    /// Routes through `batch_append_task_logs` so callers always use the optimized path.
    async fn append_task_log(&self, task_id: Uuid, chunk: &str) -> anyhow::Result<()> {
        self.batch_append_task_logs(&[(task_id, chunk.to_owned())])
            .await
    }
    async fn update_task_output(
        &self,
        task_id: Uuid,
        output: serde_json::Value,
    ) -> anyhow::Result<()>;

    /// Store the latest in-task progress snapshot for a running task.
    async fn update_task_progress(
        &self,
        task_id: Uuid,
        progress: serde_json::Value,
    ) -> anyhow::Result<()>;
    async fn reset_task_for_retry(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()>;

    /// Get pending tasks with workflow info in a single query (avoids N+1)
    /// Used by scheduler for efficient task dispatching
    async fn get_pending_tasks_with_workflow(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<TaskWithWorkflow>>;

    /// Fetch outputs for a set of task names in a run.
    async fn get_task_outputs(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> anyhow::Result<HashMap<String, serde_json::Value>>;

    /// Fetch retry metadata for tasks.
    async fn get_task_retry_meta(
        &self,
        task_ids: &[Uuid],
    ) -> anyhow::Result<HashMap<Uuid, TaskRetryMeta>>;

    /// Get run ID for a task (used by scheduler to check run completion)
    async fn get_task_run_id(&self, task_id: Uuid) -> anyhow::Result<Uuid>;

    /// Get run ID + task name for a task (used for dependency propagation)
    async fn get_task_identity(&self, task_id: Uuid) -> anyhow::Result<(Uuid, String)>;

    /// Mark pending tasks as failed if they depend on any of the given task names.
    /// Returns the names of tasks updated (used for transitive failure propagation).
    async fn mark_tasks_failed_by_dependency(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> anyhow::Result<Vec<String>>;

    /// Upsert persisted last-known task state for a workflow task (materialization-like metadata).
    async fn upsert_workflow_task_state(
        &self,
        workflow_id: Uuid,
        task_name: &str,
        last_status: TaskStatus,
        last_run_id: Uuid,
        last_task_id: Uuid,
        last_output: Option<serde_json::Value>,
        last_error: Option<&str>,
        last_finished_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()>;

    /// Batch upsert last-known task states for multiple workflow tasks in a single round-trip.
    async fn batch_upsert_workflow_task_states(
        &self,
        entries: &[WorkflowTaskStateEntry],
    ) -> anyhow::Result<()>;

    /// List persisted last-known task states for all tasks in a workflow.
    async fn list_workflow_task_states(
        &self,
        workflow_id: Uuid,
    ) -> anyhow::Result<Vec<WorkflowTaskState>>;
}

/// Repository for schedule operations
#[async_trait]
pub trait ScheduleRepository: Send + Sync {
    /// Get workflows that have schedules enabled and are due for triggering
    async fn get_due_scheduled_workflows(&self) -> anyhow::Result<Vec<Workflow>>;

    /// Update schedule trigger times for a workflow
    async fn update_workflow_schedule_times(
        &self,
        workflow_id: Uuid,
        last_scheduled_at: chrono::DateTime<chrono::Utc>,
        next_scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<()>;

    /// Update workflow schedule settings
    async fn update_workflow_schedule(
        &self,
        workflow_id: Uuid,
        schedule: Option<&str>,
        schedule_enabled: bool,
    ) -> anyhow::Result<()>;
}

/// Repository for deferred job operations (for Triggerer component)
#[async_trait]
pub trait DeferredJobRepository: Send + Sync {
    /// Create a deferred job for tracking
    async fn create_deferred_job(
        &self,
        task_id: Uuid,
        service_type: &str,
        job_id: &str,
        job_data: serde_json::Value,
    ) -> anyhow::Result<DeferredJob>;

    /// Get all deferred jobs that need polling (pending or polling status)
    async fn get_pending_deferred_jobs(&self) -> anyhow::Result<Vec<DeferredJob>>;

    /// Get deferred jobs for a specific task
    async fn get_deferred_jobs_for_task(&self, task_id: Uuid) -> anyhow::Result<Vec<DeferredJob>>;

    /// Update deferred job status
    async fn update_deferred_job_status(
        &self,
        job_id: Uuid,
        status: DeferredJobStatus,
        error: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Update deferred job last polled timestamp
    async fn update_deferred_job_polled(&self, job_id: Uuid) -> anyhow::Result<()>;

    /// Mark deferred job as completed
    async fn complete_deferred_job(&self, job_id: Uuid) -> anyhow::Result<()>;

    /// Mark deferred job as failed
    async fn fail_deferred_job(&self, job_id: Uuid, error: &str) -> anyhow::Result<()>;

    /// Cancel all deferred jobs for a task
    async fn cancel_deferred_jobs_for_task(&self, task_id: Uuid) -> anyhow::Result<()>;
}

/// Database interface for orchestration state management
///
/// This trait abstracts over different database backends (Postgres, SQLite, etc.)
/// All operations are transactional where appropriate.
///
/// This is a composite trait that combines all repository traits for backward compatibility.
/// New code should depend on the specific repository traits needed.
#[async_trait]
pub trait Database:
    WorkflowRepository
    + WorkflowSnapshotRepository
    + RunRepository
    + TaskRepository
    + ScheduleRepository
    + DeferredJobRepository
    + Send
    + Sync
{
    // Migration operations
    async fn run_migrations(&self) -> anyhow::Result<()>;
}
