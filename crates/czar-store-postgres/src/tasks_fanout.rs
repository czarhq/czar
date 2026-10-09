use anyhow::{Result, anyhow};
use sqlx::Row;
use uuid::Uuid;

use czar_core::database::NewTask;
use czar_core::models::TaskStatus;

use super::PostgresDatabase;

impl PostgresDatabase {
    pub(super) async fn replace_pending_task_impl(
        &self,
        task_id: Uuid,
        original_task_name: &str,
        replacement_tasks: &[NewTask],
    ) -> Result<()> {
        let Some(first) = replacement_tasks.first() else {
            return Err(anyhow!(
                "replace_pending_task requires at least one replacement task"
            ));
        };

        let replacement_names: Vec<String> = replacement_tasks
            .iter()
            .map(|task| task.task_name.clone())
            .collect();

        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT run_id, status FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| anyhow!("pending task {} not found", task_id))?;

        let run_id: Uuid = row.get("run_id");
        let status: TaskStatus = row.get("status");
        if !matches!(status, TaskStatus::Pending) {
            return Err(anyhow!(
                "task {} is not pending and cannot be replaced",
                task_id
            ));
        }

        let rows = sqlx::query("SELECT id, depends_on FROM tasks WHERE run_id = $1")
            .bind(run_id)
            .fetch_all(&mut *tx)
            .await?;
        for dep_row in rows {
            let dep_task_id: Uuid = dep_row.get("id");
            let depends_on: Vec<String> = dep_row.get("depends_on");
            let rewritten = rewrite_depends_on(&depends_on, original_task_name, &replacement_names);
            if rewritten != depends_on {
                sqlx::query("UPDATE tasks SET depends_on = $1 WHERE id = $2")
                    .bind(&rewritten)
                    .bind(dep_task_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        sqlx::query(
            r#"UPDATE tasks
               SET task_index = $1, task_name = $2, executor_type = $3, depends_on = $4, status = $5,
                   attempts = 0, max_retries = $6, timeout_seconds = $7, retry_at = NULL,
                   execution_name = NULL, params = $8, output = NULL, logs = NULL, error = NULL,
                   progress = NULL, dispatched_at = NULL, started_at = NULL, finished_at = NULL
               WHERE id = $9"#,
        )
        .bind(first.task_index)
        .bind(&first.task_name)
        .bind(&first.executor_type)
        .bind(&first.depends_on)
        .bind(TaskStatus::Pending.as_str())
        .bind(first.max_retries)
        .bind(first.timeout_seconds)
        .bind(&first.params)
        .bind(task_id)
        .execute(&mut *tx)
        .await?;

        for task in &replacement_tasks[1..] {
            sqlx::query(
                r#"INSERT INTO tasks (run_id, task_index, task_name, executor_type, depends_on, status, attempts, max_retries, timeout_seconds, params)
                   VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $8, $9)"#,
            )
            .bind(run_id)
            .bind(task.task_index)
            .bind(&task.task_name)
            .bind(&task.executor_type)
            .bind(&task.depends_on)
            .bind(TaskStatus::Pending.as_str())
            .bind(task.max_retries)
            .bind(task.timeout_seconds)
            .bind(&task.params)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

fn rewrite_depends_on(
    depends_on: &[String],
    original_task_name: &str,
    replacement_names: &[String],
) -> Vec<String> {
    let mut rewritten = Vec::with_capacity(depends_on.len().max(replacement_names.len()));
    for dep in depends_on {
        if dep == original_task_name {
            rewritten.extend(replacement_names.iter().cloned());
        } else {
            rewritten.push(dep.clone());
        }
    }
    rewritten
}
