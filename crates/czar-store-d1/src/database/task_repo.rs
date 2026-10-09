use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use czar_core::database::{NewTask, TaskRepository};
use czar_core::models::{Task, TaskStatus, TaskWithWorkflow, WorkflowTaskState};

use crate::database::D1Database;

#[async_trait]
impl TaskRepository for D1Database {
    async fn batch_create_tasks(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> Result<()> {
        self.batch_create_tasks_impl(run_id, task_count, workflow_name, executor_type)
            .await
    }

    async fn batch_create_dag_tasks(&self, run_id: Uuid, tasks: &[NewTask]) -> Result<()> {
        self.batch_create_dag_tasks_impl(run_id, tasks).await
    }

    async fn replace_pending_task(
        &self,
        task_id: Uuid,
        original_task_name: &str,
        replacement_tasks: &[NewTask],
    ) -> Result<()> {
        self.replace_pending_task_impl(task_id, original_task_name, replacement_tasks)
            .await
    }

    async fn update_task_status(
        &self,
        task_id: Uuid,
        status: TaskStatus,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        self.update_task_status_impl(task_id, status, execution_name, error)
            .await
    }

    async fn batch_update_task_status(
        &self,
        updates: &[(Uuid, TaskStatus, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        self.batch_update_task_status_impl(updates).await
    }

    async fn list_tasks(&self, run_id: Uuid) -> Result<Vec<Task>> {
        self.list_tasks_impl(run_id).await
    }

    async fn get_pending_tasks(&self) -> Result<Vec<Task>> {
        self.get_pending_tasks_impl().await
    }

    async fn get_running_tasks(&self) -> Result<Vec<Task>> {
        self.get_running_tasks_impl().await
    }

    async fn batch_append_task_logs(&self, entries: &[(Uuid, String)]) -> Result<()> {
        for (task_id, chunk) in entries {
            self.append_task_log_impl(*task_id, chunk).await?;
        }
        Ok(())
    }

    async fn update_task_output(&self, task_id: Uuid, output: Value) -> Result<()> {
        self.update_task_output_impl(task_id, output).await
    }

    async fn update_task_progress(&self, task_id: Uuid, progress: Value) -> Result<()> {
        self.update_task_progress_impl(task_id, progress).await
    }

    async fn reset_task_for_retry(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        self.reset_task_for_retry_impl(task_id, error, retry_at)
            .await
    }

    async fn get_pending_tasks_with_workflow(&self, limit: i64) -> Result<Vec<TaskWithWorkflow>> {
        self.get_pending_tasks_with_workflow_impl(limit).await
    }

    async fn get_task_outputs(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> Result<HashMap<String, Value>> {
        self.get_task_outputs_impl(run_id, task_names).await
    }

    async fn get_task_retry_meta(
        &self,
        task_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, czar_core::database::TaskRetryMeta>> {
        self.get_task_retry_meta_impl(task_ids).await
    }

    async fn get_task_run_id(&self, task_id: Uuid) -> Result<Uuid> {
        self.get_task_run_id_impl(task_id).await
    }

    async fn get_task_identity(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        self.get_task_identity_impl(task_id).await
    }

    async fn mark_tasks_failed_by_dependency(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> Result<Vec<String>> {
        self.mark_tasks_failed_by_dependency_impl(run_id, failed_task_names, error)
            .await
    }

    async fn upsert_workflow_task_state(
        &self,
        workflow_id: Uuid,
        task_name: &str,
        last_status: TaskStatus,
        last_run_id: Uuid,
        last_task_id: Uuid,
        last_output: Option<Value>,
        last_error: Option<&str>,
        last_finished_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        self.upsert_workflow_task_state_impl(
            workflow_id,
            task_name,
            last_status,
            last_run_id,
            last_task_id,
            last_output,
            last_error,
            last_finished_at,
        )
        .await
    }

    async fn batch_upsert_workflow_task_states(
        &self,
        entries: &[czar_core::database::WorkflowTaskStateEntry],
    ) -> Result<()> {
        for entry in entries {
            self.upsert_workflow_task_state_impl(
                entry.workflow_id,
                &entry.task_name,
                entry.last_status,
                entry.last_run_id,
                entry.last_task_id,
                entry.last_output.clone(),
                entry.last_error.as_deref(),
                entry.last_finished_at,
            )
            .await?;
        }
        Ok(())
    }

    async fn list_workflow_task_states(&self, workflow_id: Uuid) -> Result<Vec<WorkflowTaskState>> {
        self.list_workflow_task_states_impl(workflow_id).await
    }
}
