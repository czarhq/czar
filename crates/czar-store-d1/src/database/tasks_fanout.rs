use anyhow::{Result, anyhow};
use uuid::Uuid;

use czar_core::database::NewTask;
use czar_core::models::TaskStatus;

use crate::client::QueryParam;
use crate::database::{D1Database, json_text, opt_i32_param, uuid_param};

impl D1Database {
    pub(crate) async fn replace_pending_task_impl(
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

        let existing = self
            .fetch_all(
                "SELECT * FROM tasks WHERE id = ?",
                vec![uuid_param(task_id)],
                crate::decode::task,
            )
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("pending task {} not found", task_id))?;
        if !matches!(existing.status(), TaskStatus::Pending) {
            return Err(anyhow!(
                "task {} is not pending and cannot be replaced",
                task_id
            ));
        }

        let tasks_in_run = self.list_tasks_impl(existing.run_id).await?;
        for task in &tasks_in_run {
            let rewritten =
                rewrite_depends_on(&task.depends_on, original_task_name, &replacement_names);
            if rewritten != task.depends_on {
                self.execute(
                    "UPDATE tasks SET depends_on = ? WHERE id = ?",
                    vec![json_text(&rewritten).into(), uuid_param(task.id)],
                )
                .await?;
            }
        }

        self.execute(
            r#"UPDATE tasks
               SET task_index = ?, task_name = ?, executor_type = ?, depends_on = ?, status = ?,
                   attempts = 0, max_retries = ?, timeout_seconds = ?, retry_at = NULL,
                   execution_name = NULL, params = ?, output = NULL, logs = NULL, error = NULL,
                   progress = NULL, dispatched_at = NULL, started_at = NULL, finished_at = NULL
               WHERE id = ?"#,
            vec![
                first.task_index.into(),
                first.task_name.clone().into(),
                first.executor_type.clone().into(),
                json_text(&first.depends_on).into(),
                TaskStatus::Pending.as_str().into(),
                first.max_retries.into(),
                opt_i32_param(first.timeout_seconds),
                QueryParam::Text(first.params.clone().to_string()),
                uuid_param(task_id),
            ],
        )
        .await?;

        for task in &replacement_tasks[1..] {
            self.execute(
                r#"INSERT INTO tasks (id, run_id, task_index, task_name, executor_type, depends_on, status, attempts, max_retries, timeout_seconds, params)
                   VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?)"#,
                vec![
                    uuid_param(Uuid::new_v4()),
                    uuid_param(existing.run_id),
                    task.task_index.into(),
                    task.task_name.clone().into(),
                    task.executor_type.clone().into(),
                    json_text(&task.depends_on).into(),
                    TaskStatus::Pending.as_str().into(),
                    task.max_retries.into(),
                    opt_i32_param(task.timeout_seconds),
                    QueryParam::Text(task.params.clone().to_string()),
                ],
            )
            .await?;
        }

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
