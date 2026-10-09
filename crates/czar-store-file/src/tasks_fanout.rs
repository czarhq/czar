use anyhow::{Result, anyhow};
use chrono::Utc;
use uuid::Uuid;

use czar_core::database::NewTask;
use czar_core::models::{Task, TaskStatus};

use super::FileDatabase;

impl FileDatabase {
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

        let path = self.task_path(task_id);
        let existing: Task = self.read_json(&path).await?;
        if !matches!(existing.status(), TaskStatus::Pending) {
            return Err(anyhow!(
                "task {} is not pending and cannot be replaced",
                task_id
            ));
        }

        let replacement_names: Vec<String> = replacement_tasks
            .iter()
            .map(|task| task.task_name.clone())
            .collect();

        let tasks_in_run = self.list_tasks_impl(existing.run_id).await?;
        for task in tasks_in_run {
            let rewritten =
                rewrite_depends_on(&task.depends_on, original_task_name, &replacement_names);
            if rewritten == task.depends_on {
                continue;
            }
            let updated = Task {
                depends_on: rewritten,
                ..task
            };
            self.write_json(&self.task_path(updated.id), &updated)
                .await?;
        }

        let now = Utc::now();
        let updated_first = Task {
            id: existing.id,
            run_id: existing.run_id,
            task_index: first.task_index,
            task_name: first.task_name.clone(),
            executor_type: first.executor_type.clone(),
            depends_on: first.depends_on.clone(),
            status: TaskStatus::Pending,
            attempts: 0,
            max_retries: first.max_retries,
            timeout_seconds: first.timeout_seconds,
            retry_at: None,
            execution_name: None,
            params: Some(first.params.clone().into()),
            output: None,
            logs: None,
            error: None,
            progress: None,
            dispatched_at: None,
            started_at: None,
            finished_at: None,
            created_at: existing.created_at,
        };
        self.write_json(&path, &updated_first).await?;

        for task in &replacement_tasks[1..] {
            let new_task = Task {
                id: Uuid::new_v4(),
                run_id: existing.run_id,
                task_index: task.task_index,
                task_name: task.task_name.clone(),
                executor_type: task.executor_type.clone(),
                depends_on: task.depends_on.clone(),
                status: TaskStatus::Pending,
                attempts: 0,
                max_retries: task.max_retries,
                timeout_seconds: task.timeout_seconds,
                retry_at: None,
                execution_name: None,
                params: Some(task.params.clone().into()),
                output: None,
                logs: None,
                error: None,
                progress: None,
                dispatched_at: None,
                started_at: None,
                finished_at: None,
                created_at: now,
            };
            self.write_json(&self.task_path(new_task.id), &new_task)
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
