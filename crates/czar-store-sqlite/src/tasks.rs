use anyhow::Result;
use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use czar_core::database::{NewTask, TaskRetryMeta};
use czar_core::models::{RunStatus, Task, TaskStatus, TaskWithWorkflow, WorkflowTaskState};

use super::core::{SqliteDatabase, TaskRow, TaskWithWorkflowRow, encode_depends_on};

impl SqliteDatabase {
    pub(super) async fn batch_create_tasks_impl(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for i in 0..task_count {
            let task_id = Uuid::new_v4();
            let params_json = sqlx::types::Json(serde_json::json!({
                "task_index": i, "run_id": run_id, "workflow_name": workflow_name,
            }));
            sqlx::query(
                r#"INSERT INTO tasks (id, run_id, task_index, task_name, executor_type, depends_on, status, params)
                VALUES (?, ?, ?, ?, ?, '[]', ?, ?)
                ON CONFLICT (run_id, task_index) DO NOTHING"#,
            )
            .bind(task_id)
            .bind(run_id)
            .bind(i)
            .bind(format!("task_{}", i))
            .bind(executor_type)
            .bind(TaskStatus::Pending.as_str())
            .bind(params_json)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn batch_create_dag_tasks_impl(
        &self,
        run_id: Uuid,
        tasks: &[NewTask],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for task in tasks {
            let task_id = Uuid::new_v4();
            let depends_on_json = encode_depends_on(&task.depends_on);
            let params_json = sqlx::types::Json(&task.params);
            sqlx::query(
                r#"INSERT INTO tasks (id, run_id, task_index, task_name, executor_type, depends_on, status, attempts, max_retries, timeout_seconds, params)
                VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?)
                ON CONFLICT (run_id, task_index) DO NOTHING"#,
            )
            .bind(task_id)
            .bind(run_id)
            .bind(task.task_index)
            .bind(&task.task_name)
            .bind(&task.executor_type)
            .bind(&depends_on_json)
            .bind(TaskStatus::Pending.as_str())
            .bind(task.max_retries)
            .bind(task.timeout_seconds)
            .bind(params_json)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn batch_create_dag_tasks_multi_impl(
        &self,
        runs: &[(Uuid, &[NewTask])],
    ) -> Result<()> {
        for (run_id, tasks) in runs {
            self.batch_create_dag_tasks_impl(*run_id, tasks).await?;
        }
        Ok(())
    }

    pub(super) async fn update_task_status_impl(
        &self,
        task_id: Uuid,
        status: TaskStatus,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let status_str = status.as_str();
        sqlx::query(
            r#"UPDATE tasks SET status = ?, execution_name = COALESCE(?, execution_name), error = COALESCE(?, error),
            attempts = CASE WHEN ? = ? THEN attempts + 1 ELSE attempts END,
            retry_at = CASE WHEN ? IN (?, ?) THEN retry_at ELSE NULL END,
            dispatched_at = CASE WHEN ? = ? AND dispatched_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE dispatched_at END,
            started_at = CASE WHEN ? IN (?, ?, ?, ?) AND started_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END,
            finished_at = CASE WHEN ? IN (?, ?, ?) AND finished_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE finished_at END
            WHERE id = ?"#,
        )
        .bind(status_str)
        .bind(execution_name)
        .bind(error)
        .bind(status_str)
        .bind(TaskStatus::Failed.as_str())
        .bind(status_str)
        .bind(TaskStatus::Pending.as_str())
        .bind(TaskStatus::Paused.as_str())
        .bind(status_str)
        .bind(TaskStatus::Dispatched.as_str())
        .bind(status_str)
        .bind(TaskStatus::Running.as_str())
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Failed.as_str())
        .bind(TaskStatus::Cancelled.as_str())
        .bind(status_str)
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Failed.as_str())
        .bind(TaskStatus::Cancelled.as_str())
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn batch_update_task_status_impl(
        &self,
        updates: &[(Uuid, TaskStatus, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for (task_id, status, execution_name, error) in updates {
            self.update_task_status_impl_tx(&mut tx, *task_id, *status, *execution_name, *error)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn update_task_status_impl_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        task_id: Uuid,
        status: TaskStatus,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let status_str = status.as_str();
        sqlx::query(
            r#"UPDATE tasks SET status = ?, execution_name = COALESCE(?, execution_name), error = COALESCE(?, error),
            attempts = CASE WHEN ? = ? THEN attempts + 1 ELSE attempts END,
            retry_at = CASE WHEN ? IN (?, ?) THEN retry_at ELSE NULL END,
            dispatched_at = CASE WHEN ? = ? AND dispatched_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE dispatched_at END,
            started_at = CASE WHEN ? IN (?, ?, ?, ?) AND started_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END,
            finished_at = CASE WHEN ? IN (?, ?, ?) AND finished_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE finished_at END
            WHERE id = ?"#,
        )
        .bind(status_str)
        .bind(execution_name)
        .bind(error)
        .bind(status_str)
        .bind(TaskStatus::Failed.as_str())
        .bind(status_str)
        .bind(TaskStatus::Pending.as_str())
        .bind(TaskStatus::Paused.as_str())
        .bind(status_str)
        .bind(TaskStatus::Dispatched.as_str())
        .bind(status_str)
        .bind(TaskStatus::Running.as_str())
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Failed.as_str())
        .bind(TaskStatus::Cancelled.as_str())
        .bind(status_str)
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Failed.as_str())
        .bind(TaskStatus::Cancelled.as_str())
        .bind(task_id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub(super) async fn list_tasks_impl(&self, run_id: Uuid) -> Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE run_id = ? ORDER BY task_index",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Self::map_task).collect())
    }

    pub(super) async fn get_pending_tasks_impl(&self) -> Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, TaskRow>("SELECT * FROM tasks WHERE status = ?")
            .bind(TaskStatus::Pending.as_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(Self::map_task).collect())
    }

    pub(super) async fn get_running_tasks_impl(&self) -> Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, TaskRow>("SELECT * FROM tasks WHERE status IN (?, ?)")
            .bind(TaskStatus::Running.as_str())
            .bind(TaskStatus::Dispatched.as_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(Self::map_task).collect())
    }

    pub(super) async fn append_task_log_impl(&self, task_id: Uuid, chunk: &str) -> Result<()> {
        sqlx::query(
            "UPDATE tasks SET logs = COALESCE(SUBSTR(logs, 1, 2000000), '') || ? WHERE id = ?",
        )
        .bind(chunk)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn batch_append_task_logs_impl(
        &self,
        entries: &[(Uuid, String)],
    ) -> Result<()> {
        for (task_id, chunk) in entries {
            self.append_task_log_impl(*task_id, chunk).await?;
        }
        Ok(())
    }

    pub(super) async fn update_task_output_impl(
        &self,
        task_id: Uuid,
        output: serde_json::Value,
    ) -> Result<()> {
        sqlx::query("UPDATE tasks SET output = ? WHERE id = ?")
            .bind(sqlx::types::Json(output))
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(super) async fn update_task_progress_impl(
        &self,
        task_id: Uuid,
        progress: serde_json::Value,
    ) -> Result<()> {
        sqlx::query("UPDATE tasks SET progress = ? WHERE id = ?")
            .bind(sqlx::types::Json(progress))
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(super) async fn reset_task_for_retry_impl(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<chrono::DateTime<Utc>>,
    ) -> Result<()> {
        let retry_str = retry_at.map(|dt| dt.to_rfc3339_opts(SecondsFormat::Secs, true));
        sqlx::query(
            r#"UPDATE tasks SET status = ?, attempts = attempts + 1, retry_at = ?,
            error = COALESCE(?, error), execution_name = NULL, output = NULL, dispatched_at = NULL, started_at = NULL, finished_at = NULL
            WHERE id = ?"#,
        )
        .bind(TaskStatus::Pending.as_str())
        .bind(retry_str)
        .bind(error)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn get_pending_tasks_with_workflow_impl(
        &self,
        limit: i64,
    ) -> Result<Vec<TaskWithWorkflow>> {
        let rows = sqlx::query_as::<_, TaskWithWorkflowRow>(
            r#"SELECT t.id as task_id, t.run_id, t.task_index, t.task_name, t.executor_type, t.depends_on,
            t.status as task_status, t.attempts, t.max_retries, t.timeout_seconds, t.retry_at, t.execution_name, t.params,
            w.id as workflow_id, w.job_name, w.project, w.region
            FROM tasks t JOIN runs r ON t.run_id = r.id JOIN workflows w ON r.workflow_id = w.id
            WHERE t.status = ?
            AND r.status = ?
            AND (t.retry_at IS NULL OR datetime(t.retry_at) <= datetime('now'))
            AND (t.depends_on = '[]' OR NOT EXISTS (
                SELECT 1 FROM json_each(t.depends_on) AS dep
                JOIN tasks t2
                  ON t2.run_id = t.run_id
                 AND t2.task_name = dep.value
                WHERE t2.status NOT IN (?, ?, ?)
            ))
            ORDER BY t.created_at LIMIT ?"#,
        )
        .bind(TaskStatus::Pending.as_str())
        .bind(RunStatus::Running.as_str())
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Failed.as_str())
        .bind(TaskStatus::Skipped.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Self::map_task_with_workflow).collect())
    }

    pub(super) async fn get_task_outputs_impl(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        if task_names.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = task_names.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "SELECT task_name, output FROM tasks WHERE run_id = ? AND task_name IN ({}) AND output IS NOT NULL",
            placeholders
        );
        let mut query = sqlx::query(sqlx::AssertSqlSafe(query_str)).bind(run_id);
        for name in task_names {
            query = query.bind(name);
        }
        let rows = query.fetch_all(&self.pool).await?;
        let mut map = HashMap::new();
        for row in rows {
            let name: String = row.try_get("task_name")?;
            let output: sqlx::types::Json<serde_json::Value> = row.try_get("output")?;
            map.insert(name, output.0);
        }
        Ok(map)
    }

    pub(super) async fn get_task_retry_meta_impl(
        &self,
        task_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, TaskRetryMeta>> {
        if task_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "SELECT id, attempts, max_retries, params FROM tasks WHERE id IN ({})",
            placeholders
        );
        let mut query = sqlx::query(sqlx::AssertSqlSafe(query_str));
        for id in task_ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(&self.pool).await?;
        let mut map = HashMap::new();
        for row in rows {
            let id: Uuid = row.get("id");
            let attempts: i32 = row.get("attempts");
            let max_retries: i32 = row.get("max_retries");
            let params: Option<sqlx::types::Json<serde_json::Value>> = row.try_get("params")?;
            let retry_delay_seconds = params
                .as_ref()
                .and_then(|value| value.0.get("retry_delay_seconds"))
                .and_then(|value| value.as_i64())
                .map(|value| value.clamp(0, i64::from(i32::MAX)) as i32);
            map.insert(
                id,
                TaskRetryMeta {
                    attempts,
                    max_retries,
                    retry_delay_seconds,
                },
            );
        }
        Ok(map)
    }

    pub(super) async fn get_task_run_id_impl(&self, task_id: Uuid) -> Result<Uuid> {
        let row = sqlx::query("SELECT run_id FROM tasks WHERE id = ?")
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get("run_id"))
    }

    pub(super) async fn get_task_identity_impl(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        let row = sqlx::query("SELECT run_id, task_name FROM tasks WHERE id = ?")
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?;
        Ok((row.get("run_id"), row.get("task_name")))
    }

    pub(super) async fn mark_tasks_failed_by_dependency_impl(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> Result<Vec<String>> {
        if failed_task_names.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = failed_task_names
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let query_str = format!(
            r#"UPDATE tasks SET status = ?, error = ?, finished_at = CURRENT_TIMESTAMP
            WHERE run_id = ? AND status IN (?, ?) AND EXISTS (
                SELECT 1 FROM json_each(depends_on) WHERE value IN ({})
            ) RETURNING task_name"#,
            placeholders
        );
        let mut query = sqlx::query(sqlx::AssertSqlSafe(query_str))
            .bind(TaskStatus::Failed.as_str())
            .bind(error)
            .bind(run_id)
            .bind(TaskStatus::Pending.as_str())
            .bind(TaskStatus::Paused.as_str());
        for name in failed_task_names {
            query = query.bind(name);
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|row| row.get("task_name")).collect())
    }

    pub(super) async fn upsert_workflow_task_state_impl(
        &self,
        workflow_id: Uuid,
        task_name: &str,
        last_status: TaskStatus,
        last_run_id: Uuid,
        last_task_id: Uuid,
        last_output: Option<serde_json::Value>,
        last_error: Option<&str>,
        last_finished_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let status_str = last_status.as_str();
        let finished_at_str =
            last_finished_at.map(|dt| dt.to_rfc3339_opts(SecondsFormat::Secs, true));
        sqlx::query(
            r#"INSERT INTO workflow_task_states (
                id, workflow_id, task_name, last_status, last_run_id, last_task_id,
                last_output, last_error, last_finished_at, last_success_at, last_failure_at, updated_at
            ) VALUES (
                ?, ?, ?, ?, ?, ?,
                ?, ?, ?,
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
        )
        .bind(Uuid::new_v4())
        .bind(workflow_id)
        .bind(task_name)
        .bind(status_str)
        .bind(last_run_id)
        .bind(last_task_id)
        .bind(last_output.map(sqlx::types::Json))
        .bind(last_error)
        .bind(finished_at_str.clone())
        .bind(status_str)
        .bind(finished_at_str.clone())
        .bind(status_str)
        .bind(finished_at_str)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn batch_upsert_workflow_task_states_impl(
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

    pub(super) async fn list_workflow_task_states_impl(
        &self,
        workflow_id: Uuid,
    ) -> Result<Vec<WorkflowTaskState>> {
        let states = sqlx::query_as::<_, WorkflowTaskState>(
            "SELECT * FROM workflow_task_states WHERE workflow_id = ? ORDER BY task_name",
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(states)
    }
}
