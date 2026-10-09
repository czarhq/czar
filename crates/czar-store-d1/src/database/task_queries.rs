use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use czar_core::database::TaskRetryMeta;
use czar_core::models::{TaskStatus, Workflow, WorkflowTaskState};

use crate::database::{D1Database, decode, opt_datetime, opt_json, opt_text, uuid_param};

impl D1Database {
    pub(crate) async fn get_task_outputs_impl(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> Result<HashMap<String, Value>> {
        if task_names.is_empty() {
            return Ok(HashMap::new());
        }

        let mut params = vec![uuid_param(run_id)];
        let placeholders = task_names
            .iter()
            .map(|task_name| {
                params.push(task_name.clone().into());
                "?"
            })
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT task_name, output FROM tasks WHERE run_id = ? AND task_name IN ({placeholders}) AND output IS NOT NULL"
        );
        let rows = self.fetch_all(&sql, params, Ok).await?;
        let mut outputs = HashMap::with_capacity(rows.len());
        for row in rows {
            outputs.insert(
                decode::string(&row, "task_name")?,
                decode::json(&row, "output")?,
            );
        }
        Ok(outputs)
    }

    pub(crate) async fn get_task_retry_meta_impl(
        &self,
        task_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, TaskRetryMeta>> {
        if task_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut params = Vec::with_capacity(task_ids.len());
        let placeholders = task_ids
            .iter()
            .map(|task_id| {
                params.push(uuid_param(*task_id));
                "?"
            })
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, attempts, max_retries, params FROM tasks WHERE id IN ({placeholders})"
        );
        let rows = self.fetch_all(&sql, params, Ok).await?;
        let mut meta = HashMap::with_capacity(rows.len());
        for row in rows {
            meta.insert(
                decode::uuid(&row, "id")?,
                TaskRetryMeta {
                    attempts: decode::i32_value(&row, "attempts")?,
                    max_retries: decode::i32_value(&row, "max_retries")?,
                    retry_delay_seconds: decode::opt_json(&row, "params")?
                        .as_ref()
                        .and_then(|value| value.get("retry_delay_seconds"))
                        .and_then(|value| value.as_i64())
                        .map(|value| value.clamp(0, i64::from(i32::MAX)) as i32),
                },
            );
        }
        Ok(meta)
    }

    pub(crate) async fn get_task_run_id_impl(&self, task_id: Uuid) -> Result<Uuid> {
        self.fetch_one(
            "SELECT run_id FROM tasks WHERE id = ?",
            vec![uuid_param(task_id)],
            |row| decode::uuid(&row, "run_id"),
        )
        .await
    }

    pub(crate) async fn get_task_identity_impl(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        self.fetch_one(
            "SELECT run_id, task_name FROM tasks WHERE id = ?",
            vec![uuid_param(task_id)],
            |row| {
                Ok((
                    decode::uuid(&row, "run_id")?,
                    decode::string(&row, "task_name")?,
                ))
            },
        )
        .await
    }

    pub(crate) async fn mark_tasks_failed_by_dependency_impl(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> Result<Vec<String>> {
        if failed_task_names.is_empty() {
            return Ok(Vec::new());
        }

        let mut params = vec![
            TaskStatus::Failed.as_str().into(),
            error.into(),
            uuid_param(run_id),
            TaskStatus::Pending.as_str().into(),
            TaskStatus::Paused.as_str().into(),
        ];
        let placeholders = failed_task_names
            .iter()
            .map(|task_name| {
                params.push(task_name.clone().into());
                "?"
            })
            .collect::<Vec<_>>()
            .join(",");

        let sql = format!(
            r#"UPDATE tasks SET status = ?, error = ?, finished_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')
               WHERE run_id = ? AND status IN (?, ?) AND EXISTS (
                   SELECT 1 FROM json_each(depends_on) WHERE value IN ({placeholders})
               ) RETURNING task_name"#
        );
        let rows = self.fetch_all(&sql, params, Ok).await?;
        rows.into_iter()
            .map(|row| decode::string(&row, "task_name"))
            .collect()
    }

    pub(crate) async fn upsert_workflow_task_state_impl(
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
        let status_str = last_status.as_str();
        let finished_at = opt_datetime(last_finished_at);
        self.execute(
            r#"INSERT INTO workflow_task_states (
                id, workflow_id, task_name, last_status, last_run_id, last_task_id,
                last_output, last_error, last_finished_at, last_success_at, last_failure_at, updated_at
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?,
                CASE WHEN ? = 'success' THEN ? ELSE NULL END,
                CASE WHEN ? IN ('failed', 'cancelled') THEN ? ELSE NULL END,
                STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')
            )
            ON CONFLICT(workflow_id, task_name) DO UPDATE SET
                last_status = excluded.last_status,
                last_run_id = excluded.last_run_id,
                last_task_id = excluded.last_task_id,
                last_output = excluded.last_output,
                last_error = excluded.last_error,
                last_finished_at = excluded.last_finished_at,
                last_success_at = CASE
                    WHEN excluded.last_status = 'success' THEN excluded.last_finished_at
                    ELSE workflow_task_states.last_success_at
                END,
                last_failure_at = CASE
                    WHEN excluded.last_status IN ('failed', 'cancelled') THEN excluded.last_finished_at
                    ELSE workflow_task_states.last_failure_at
                END,
                updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')"#,
            vec![
                uuid_param(Uuid::new_v4()),
                uuid_param(workflow_id),
                task_name.into(),
                status_str.into(),
                uuid_param(last_run_id),
                uuid_param(last_task_id),
                opt_json(last_output),
                opt_text(last_error),
                finished_at.clone(),
                status_str.into(),
                finished_at.clone(),
                status_str.into(),
                finished_at,
            ],
        )
        .await
    }

    pub(crate) async fn list_workflow_task_states_impl(
        &self,
        workflow_id: Uuid,
    ) -> Result<Vec<WorkflowTaskState>> {
        self.fetch_all(
            "SELECT * FROM workflow_task_states WHERE workflow_id = ? ORDER BY task_name",
            vec![uuid_param(workflow_id)],
            decode::workflow_task_state,
        )
        .await
    }

    pub(crate) async fn get_due_scheduled_workflows_impl(&self) -> Result<Vec<Workflow>> {
        self.fetch_all(
            r#"SELECT * FROM workflows
               WHERE schedule_enabled = 1
                 AND schedule IS NOT NULL
                 AND (next_scheduled_at IS NULL OR next_scheduled_at <= ?)
               ORDER BY next_scheduled_at"#,
            vec![Utc::now().to_rfc3339().into()],
            decode::workflow,
        )
        .await
    }
}
