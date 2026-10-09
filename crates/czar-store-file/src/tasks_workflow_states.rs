use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use czar_core::database::WorkflowTaskStateEntry;
use czar_core::models::{TaskStatus, WorkflowTaskState};

use super::core::FileDatabase;

impl FileDatabase {
    pub(super) async fn upsert_workflow_task_state_impl(
        &self,
        workflow_id: Uuid,
        task_name: &str,
        last_status: TaskStatus,
        last_run_id: Uuid,
        last_task_id: Uuid,
        last_output: Option<serde_json::Value>,
        last_error: Option<&str>,
        last_finished_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        self.ensure_dirs().await?;
        let path = self.workflow_task_states_path(workflow_id);
        let mut states: Vec<WorkflowTaskState> = if path.exists() {
            self.read_json(&path).await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let now = Utc::now();
        if let Some(state) = states.iter_mut().find(|s| s.task_name == task_name) {
            state.last_status = last_status;
            state.last_run_id = last_run_id;
            state.last_task_id = last_task_id;
            state.last_output = last_output.clone().map(Into::into);
            state.last_error = last_error.map(ToOwned::to_owned);
            state.last_finished_at = last_finished_at;
            if matches!(last_status, TaskStatus::Success) {
                state.last_success_at = last_finished_at;
            }
            if matches!(last_status, TaskStatus::Failed | TaskStatus::Cancelled) {
                state.last_failure_at = last_finished_at;
            }
            state.updated_at = now;
        } else {
            states.push(WorkflowTaskState {
                id: Uuid::new_v4(),
                workflow_id,
                task_name: task_name.to_string(),
                last_status,
                last_run_id,
                last_task_id,
                last_output: last_output.map(Into::into),
                last_error: last_error.map(ToOwned::to_owned),
                last_finished_at,
                last_success_at: if matches!(last_status, TaskStatus::Success) {
                    last_finished_at
                } else {
                    None
                },
                last_failure_at: if matches!(
                    last_status,
                    TaskStatus::Failed | TaskStatus::Cancelled
                ) {
                    last_finished_at
                } else {
                    None
                },
                updated_at: now,
            });
        }

        self.write_json(&path, &states).await?;
        Ok(())
    }

    pub(super) async fn batch_upsert_workflow_task_states_impl(
        &self,
        entries: &[WorkflowTaskStateEntry],
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

    pub(super) async fn list_workflow_task_states_impl(
        &self,
        workflow_id: Uuid,
    ) -> Result<Vec<WorkflowTaskState>> {
        self.ensure_dirs().await?;
        let path = self.workflow_task_states_path(workflow_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut states: Vec<WorkflowTaskState> = self.read_json(&path).await?;
        states.sort_by(|a, b| a.task_name.cmp(&b.task_name));
        Ok(states)
    }
}
