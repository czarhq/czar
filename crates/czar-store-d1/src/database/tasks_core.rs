use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use czar_core::database::NewTask;
use czar_core::models::{RunStatus, Task, TaskStatus, TaskWithWorkflow};

use crate::client::QueryParam;
use crate::database::{
    D1Database, decode, json_text, opt_datetime, opt_i32_param, opt_text, uuid_param,
};

impl D1Database {
    pub(crate) async fn batch_create_tasks_impl(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> Result<()> {
        if task_count <= 0 {
            return Ok(());
        }

        let mut params = Vec::with_capacity(task_count as usize * 8);
        let values = (0..task_count)
            .map(|task_index| {
                params.push(uuid_param(Uuid::new_v4()));
                params.push(uuid_param(run_id));
                params.push(task_index.into());
                params.push(format!("task_{task_index}").into());
                params.push(executor_type.into());
                params.push("[]".into());
                params.push(TaskStatus::Pending.as_str().into());
                params.push(QueryParam::Text(
                    serde_json::json!({
                        "task_index": task_index,
                        "run_id": run_id,
                        "workflow_name": workflow_name,
                    })
                    .to_string(),
                ));
                "(?, ?, ?, ?, ?, ?, ?, ?)"
            })
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "INSERT INTO tasks (id, run_id, task_index, task_name, executor_type, depends_on, status, params) VALUES {values} ON CONFLICT (run_id, task_index) DO NOTHING"
        );
        self.execute(&sql, params).await
    }

    pub(crate) async fn batch_create_dag_tasks_impl(
        &self,
        run_id: Uuid,
        tasks: &[NewTask],
    ) -> Result<()> {
        if tasks.is_empty() {
            return Ok(());
        }

        let mut params = Vec::with_capacity(tasks.len() * 11);
        let values = tasks
            .iter()
            .map(|task| {
                params.push(uuid_param(Uuid::new_v4()));
                params.push(uuid_param(run_id));
                params.push(task.task_index.into());
                params.push(task.task_name.clone().into());
                params.push(task.executor_type.clone().into());
                params.push(json_text(&task.depends_on).into());
                params.push(TaskStatus::Pending.as_str().into());
                params.push(task.max_retries.into());
                params.push(opt_i32_param(task.timeout_seconds));
                params.push(QueryParam::Text(task.params.to_string()));
                params.push(0_i32.into());
                "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            })
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "INSERT INTO tasks (id, run_id, task_index, task_name, executor_type, depends_on, status, max_retries, timeout_seconds, params, attempts) VALUES {values} ON CONFLICT (run_id, task_index) DO NOTHING"
        );
        self.execute(&sql, params).await
    }

    pub(crate) async fn update_task_status_impl(
        &self,
        task_id: Uuid,
        status: TaskStatus,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let status_str = status.as_str();
        self.execute(
            r#"UPDATE tasks SET status = ?, execution_name = COALESCE(?, execution_name), error = COALESCE(?, error),
               attempts = CASE WHEN ? = ? THEN attempts + 1 ELSE attempts END,
               retry_at = CASE WHEN ? IN (?, ?) THEN retry_at ELSE NULL END,
               dispatched_at = CASE WHEN ? = ? AND dispatched_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE dispatched_at END,
               started_at = CASE WHEN ? = ? AND started_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END,
               finished_at = CASE WHEN ? IN (?, ?, ?) AND finished_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE finished_at END
               WHERE id = ?"#,
            vec![
                status_str.into(),
                opt_text(execution_name),
                opt_text(error),
                status_str.into(),
                TaskStatus::Failed.as_str().into(),
                status_str.into(),
                TaskStatus::Pending.as_str().into(),
                TaskStatus::Paused.as_str().into(),
                status_str.into(),
                TaskStatus::Dispatched.as_str().into(),
                status_str.into(),
                TaskStatus::Running.as_str().into(),
                status_str.into(),
                TaskStatus::Success.as_str().into(),
                TaskStatus::Failed.as_str().into(),
                TaskStatus::Cancelled.as_str().into(),
                uuid_param(task_id),
            ],
        )
        .await
    }

    pub(crate) async fn batch_update_task_status_impl(
        &self,
        updates: &[(Uuid, TaskStatus, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        for (task_id, status, execution_name, error) in updates {
            self.update_task_status_impl(*task_id, *status, *execution_name, *error)
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn list_tasks_impl(&self, run_id: Uuid) -> Result<Vec<Task>> {
        self.fetch_all(
            "SELECT * FROM tasks WHERE run_id = ? ORDER BY task_index",
            vec![uuid_param(run_id)],
            decode::task,
        )
        .await
    }

    pub(crate) async fn get_pending_tasks_impl(&self) -> Result<Vec<Task>> {
        self.fetch_all(
            "SELECT * FROM tasks WHERE status = ?",
            vec![TaskStatus::Pending.as_str().into()],
            decode::task,
        )
        .await
    }

    pub(crate) async fn get_running_tasks_impl(&self) -> Result<Vec<Task>> {
        self.fetch_all(
            "SELECT * FROM tasks WHERE status IN (?, ?)",
            vec![
                TaskStatus::Running.as_str().into(),
                TaskStatus::Dispatched.as_str().into(),
            ],
            decode::task,
        )
        .await
    }

    pub(crate) async fn append_task_log_impl(&self, task_id: Uuid, chunk: &str) -> Result<()> {
        self.execute(
            "UPDATE tasks SET logs = COALESCE(SUBSTR(logs, 1, 2000000), '') || ? WHERE id = ?",
            vec![chunk.into(), uuid_param(task_id)],
        )
        .await
    }

    pub(crate) async fn update_task_output_impl(&self, task_id: Uuid, output: Value) -> Result<()> {
        self.execute(
            "UPDATE tasks SET output = ? WHERE id = ?",
            vec![QueryParam::Text(output.to_string()), uuid_param(task_id)],
        )
        .await
    }

    pub(crate) async fn update_task_progress_impl(
        &self,
        task_id: Uuid,
        progress: Value,
    ) -> Result<()> {
        self.execute(
            "UPDATE tasks SET progress = ? WHERE id = ?",
            vec![QueryParam::Text(progress.to_string()), uuid_param(task_id)],
        )
        .await
    }

    pub(crate) async fn reset_task_for_retry_impl(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        self.execute(
            r#"UPDATE tasks SET status = ?, attempts = attempts + 1, retry_at = ?,
               error = COALESCE(?, error), execution_name = NULL, output = NULL,
               dispatched_at = NULL, started_at = NULL, finished_at = NULL
               WHERE id = ?"#,
            vec![
                TaskStatus::Pending.as_str().into(),
                opt_datetime(retry_at),
                opt_text(error),
                uuid_param(task_id),
            ],
        )
        .await
    }

    pub(crate) async fn get_pending_tasks_with_workflow_impl(
        &self,
        limit: i64,
    ) -> Result<Vec<TaskWithWorkflow>> {
        self.fetch_all(
            r#"SELECT t.id AS task_id, t.run_id, t.task_index, t.task_name, t.executor_type, t.depends_on,
               t.status AS task_status, t.attempts, t.max_retries, t.timeout_seconds, t.retry_at, t.execution_name, t.params,
               w.id AS workflow_id, w.job_name, w.project, w.region
               FROM tasks t
               JOIN runs r ON t.run_id = r.id
               JOIN workflows w ON r.workflow_id = w.id
               WHERE t.status = ?
                 AND r.status = ?
                 AND (t.retry_at IS NULL OR datetime(t.retry_at) <= datetime('now'))
                 AND (t.depends_on = '[]' OR NOT EXISTS (
                    SELECT 1 FROM json_each(t.depends_on) AS dep
                    JOIN tasks t2
                      ON t2.run_id = t.run_id
                     AND t2.task_name = dep.value
                    WHERE t2.status NOT IN (?, ?)
                 ))
               ORDER BY t.created_at
               LIMIT ?"#,
            vec![
                TaskStatus::Pending.as_str().into(),
                RunStatus::Running.as_str().into(),
                TaskStatus::Success.as_str().into(),
                TaskStatus::Failed.as_str().into(),
                limit.into(),
            ],
            decode::task_with_workflow,
        )
        .await
    }
}
