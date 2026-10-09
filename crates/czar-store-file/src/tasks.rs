use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use tokio::fs;
use uuid::Uuid;

use czar_core::database::{NewTask, TaskRetryMeta};
use czar_core::models::{Task, TaskStatus, TaskWithWorkflow, json_inner};

use super::core::FileDatabase;

impl FileDatabase {
    pub(super) async fn batch_create_tasks_impl(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> Result<()> {
        self.ensure_dirs().await?;
        for i in 0..task_count {
            let id = Uuid::new_v4();
            let now = Utc::now();
            let params = serde_json::json!({
                "task_index": i,
                "run_id": run_id,
                "workflow_name": workflow_name,
            });
            let task: Task = serde_json::from_value(serde_json::json!({
                "id": id,
                "run_id": run_id,
                "task_index": i,
                "task_name": format!("task_{}", i),
                "executor_type": executor_type,
                "depends_on": [],
                "status": TaskStatus::Pending,
                "attempts": 0,
                "max_retries": 0,
                "timeout_seconds": null,
                "retry_at": null,
                "execution_name": null,
                "params": params,
                "output": null,
                "logs": null,
                "error": null,
                "dispatched_at": null,
                "started_at": null,
                "finished_at": null,
                "created_at": now,
            }))?;
            self.write_json(&self.task_path(task.id), &task).await?;
        }
        Ok(())
    }

    pub(super) async fn batch_create_dag_tasks_impl(
        &self,
        run_id: Uuid,
        tasks: &[NewTask],
    ) -> Result<()> {
        self.ensure_dirs().await?;
        let now = Utc::now();
        for task in tasks {
            let task_json: Task = serde_json::from_value(serde_json::json!({
                "id": Uuid::new_v4(),
                "run_id": run_id,
                "task_index": task.task_index,
                "task_name": task.task_name,
                "executor_type": task.executor_type,
                "depends_on": task.depends_on,
                "status": TaskStatus::Pending,
                "attempts": 0,
                "max_retries": task.max_retries,
                "timeout_seconds": task.timeout_seconds,
                "retry_at": null,
                "execution_name": null,
                "params": task.params,
                "output": null,
                "logs": null,
                "error": null,
                "dispatched_at": null,
                "started_at": null,
                "finished_at": null,
                "created_at": now,
            }))?;
            self.write_json(&self.task_path(task_json.id), &task_json)
                .await?;
        }
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
        let path = self.task_path(task_id);
        let task: Task = self.read_json(&path).await?;
        let now = Utc::now();
        let dispatched_at =
            if matches!(status, TaskStatus::Dispatched) && task.dispatched_at.is_none() {
                Some(now)
            } else {
                task.dispatched_at
            };
        let started_at = if matches!(
            status,
            TaskStatus::Running | TaskStatus::Success | TaskStatus::Failed | TaskStatus::Cancelled
        ) && task.started_at.is_none()
        {
            Some(now)
        } else {
            task.started_at
        };
        let finished_at = if matches!(
            status,
            TaskStatus::Success | TaskStatus::Failed | TaskStatus::Cancelled
        ) && task.finished_at.is_none()
        {
            Some(now)
        } else {
            task.finished_at
        };
        let attempts = if matches!(status, TaskStatus::Failed) {
            task.attempts + 1
        } else {
            task.attempts
        };
        let retry_at = if matches!(status, TaskStatus::Pending | TaskStatus::Paused) {
            task.retry_at
        } else {
            None
        };
        let updated_task: Task = serde_json::from_value(serde_json::json!({
            "id": task.id,
            "run_id": task.run_id,
            "task_index": task.task_index,
            "task_name": task.task_name,
            "executor_type": task.executor_type,
            "depends_on": task.depends_on,
            "status": status,
            "attempts": attempts,
            "max_retries": task.max_retries,
            "timeout_seconds": task.timeout_seconds,
            "retry_at": retry_at,
            "execution_name": execution_name.or(task.execution_name.as_deref()),
            "params": task.params,
            "output": task.output,
            "logs": task.logs,
            "error": error.or(task.error.as_deref()),
            "dispatched_at": dispatched_at,
            "started_at": started_at,
            "finished_at": finished_at,
            "created_at": task.created_at,
        }))?;
        self.write_json(&path, &updated_task).await?;
        Ok(())
    }

    pub(super) async fn batch_update_task_status_impl(
        &self,
        updates: &[(Uuid, TaskStatus, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        for (task_id, status, execution_name, error) in updates {
            self.update_task_status_impl(*task_id, *status, *execution_name, *error)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn list_tasks_impl(&self, run_id: Uuid) -> Result<Vec<Task>> {
        self.ensure_dirs().await?;
        let mut tasks = Vec::new();
        let mut entries = fs::read_dir(self.tasks_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(task) = self.read_json::<Task>(&entry.path()).await
                && task.run_id == run_id
            {
                tasks.push(task);
            }
        }
        tasks.sort_by_key(|t| t.task_index);
        Ok(tasks)
    }

    pub(super) async fn list_tasks_all(&self) -> Result<Vec<Task>> {
        self.ensure_dirs().await?;
        let mut tasks = Vec::new();
        let mut entries = fs::read_dir(self.tasks_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(task) = self.read_json::<Task>(&entry.path()).await {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    pub(super) async fn get_pending_tasks_impl(&self) -> Result<Vec<Task>> {
        self.ensure_dirs().await?;
        let mut tasks = Vec::new();
        let mut entries = fs::read_dir(self.tasks_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(task) = self.read_json::<Task>(&entry.path()).await
                && matches!(task.status(), TaskStatus::Pending)
            {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    pub(super) async fn get_running_tasks_impl(&self) -> Result<Vec<Task>> {
        self.ensure_dirs().await?;
        let mut tasks = Vec::new();
        let mut entries = fs::read_dir(self.tasks_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(task) = self.read_json::<Task>(&entry.path()).await
                && matches!(task.status(), TaskStatus::Running | TaskStatus::Dispatched)
            {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    pub(super) async fn get_pending_tasks_with_workflow_impl(
        &self,
        _limit: i64,
    ) -> Result<Vec<TaskWithWorkflow>> {
        anyhow::bail!(
            "FileDatabase::get_pending_tasks_with_workflow not implemented; use get_pending_tasks instead"
        );
    }

    pub(super) async fn append_task_log_impl(&self, task_id: Uuid, chunk: &str) -> Result<()> {
        let path = self.task_path(task_id);
        let task: Task = self.read_json(&path).await?;
        let logs = task
            .logs
            .unwrap_or_default()
            .chars()
            .take(2_000_000)
            .collect::<String>();
        let updated_task: Task = serde_json::from_value(serde_json::json!({
            "id": task.id, "run_id": task.run_id, "task_index": task.task_index, "task_name": task.task_name,
            "executor_type": task.executor_type, "depends_on": task.depends_on, "status": task.status,
            "attempts": task.attempts, "max_retries": task.max_retries, "timeout_seconds": task.timeout_seconds,
            "retry_at": task.retry_at, "execution_name": task.execution_name, "params": task.params,
            "output": task.output, "logs": format!("{}{}", logs, chunk), "error": task.error,
            "dispatched_at": task.dispatched_at, "started_at": task.started_at,
            "finished_at": task.finished_at, "created_at": task.created_at,
        }))?;
        self.write_json(&path, &updated_task).await?;
        Ok(())
    }

    pub(super) async fn batch_append_task_logs_impl(
        &self,
        entries: &[(uuid::Uuid, String)],
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
        let path = self.task_path(task_id);
        let task: Task = self.read_json(&path).await?;
        let updated_task: Task = serde_json::from_value(serde_json::json!({
            "id": task.id, "run_id": task.run_id, "task_index": task.task_index, "task_name": task.task_name,
            "executor_type": task.executor_type, "depends_on": task.depends_on, "status": task.status,
            "attempts": task.attempts, "max_retries": task.max_retries, "timeout_seconds": task.timeout_seconds,
            "retry_at": task.retry_at, "execution_name": task.execution_name, "params": task.params,
            "output": output, "logs": task.logs, "error": task.error,
            "dispatched_at": task.dispatched_at, "started_at": task.started_at,
            "finished_at": task.finished_at, "created_at": task.created_at,
        }))?;
        self.write_json(&path, &updated_task).await?;
        Ok(())
    }

    pub(super) async fn update_task_progress_impl(
        &self,
        task_id: Uuid,
        progress: serde_json::Value,
    ) -> Result<()> {
        let path = self.task_path(task_id);
        let task: Task = self.read_json(&path).await?;
        let updated_task: Task = serde_json::from_value(serde_json::json!({
            "id": task.id, "run_id": task.run_id, "task_index": task.task_index, "task_name": task.task_name,
            "executor_type": task.executor_type, "depends_on": task.depends_on, "status": task.status,
            "attempts": task.attempts, "max_retries": task.max_retries, "timeout_seconds": task.timeout_seconds,
            "retry_at": task.retry_at, "execution_name": task.execution_name, "params": task.params,
            "output": task.output, "logs": task.logs, "error": task.error, "progress": progress,
            "dispatched_at": task.dispatched_at, "started_at": task.started_at,
            "finished_at": task.finished_at, "created_at": task.created_at,
        }))?;
        self.write_json(&path, &updated_task).await?;
        Ok(())
    }

    pub(super) async fn reset_task_for_retry_impl(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        let path = self.task_path(task_id);
        let task: Task = self.read_json(&path).await?;
        let updated_task: Task = serde_json::from_value(serde_json::json!({
            "id": task.id, "run_id": task.run_id, "task_index": task.task_index, "task_name": task.task_name,
            "executor_type": task.executor_type, "depends_on": task.depends_on, "status": TaskStatus::Pending,
            "attempts": task.attempts + 1, "max_retries": task.max_retries, "timeout_seconds": task.timeout_seconds,
            "retry_at": retry_at, "execution_name": null, "params": task.params,
            "output": null, "logs": task.logs, "error": error.or(task.error.as_deref()),
            "dispatched_at": null, "started_at": null, "finished_at": null, "created_at": task.created_at,
        }))?;
        self.write_json(&path, &updated_task).await?;
        Ok(())
    }

    pub(super) async fn get_task_outputs_impl(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        if task_names.is_empty() {
            return Ok(HashMap::new());
        }
        let tasks = self.list_tasks_impl(run_id).await?;
        let names: std::collections::HashSet<&String> = task_names.iter().collect();
        let mut map = HashMap::new();
        for task in tasks {
            if !names.contains(&task.task_name) {
                continue;
            }
            if let Some(output) = task.output {
                map.insert(task.task_name, json_inner(&output).clone());
            }
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
        let tasks = self.list_tasks_all().await?;
        let ids: std::collections::HashSet<Uuid> = task_ids.iter().copied().collect();
        let mut map = HashMap::new();
        for task in tasks {
            if ids.contains(&task.id) {
                let retry_delay_seconds = task
                    .params
                    .as_ref()
                    .and_then(|value| json_inner(value).get("retry_delay_seconds"))
                    .and_then(|value| value.as_i64())
                    .map(|value| value.clamp(0, i64::from(i32::MAX)) as i32);
                map.insert(
                    task.id,
                    TaskRetryMeta {
                        attempts: task.attempts,
                        max_retries: task.max_retries,
                        retry_delay_seconds,
                    },
                );
            }
        }
        Ok(map)
    }

    pub(super) async fn get_task_run_id_impl(&self, task_id: Uuid) -> Result<Uuid> {
        let task: Task = self.read_json(&self.task_path(task_id)).await?;
        Ok(task.run_id)
    }

    pub(super) async fn get_task_identity_impl(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        let task: Task = self.read_json(&self.task_path(task_id)).await?;
        Ok((task.run_id, task.task_name))
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
        let mut updated = Vec::new();
        let mut entries = fs::read_dir(self.tasks_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let task = match self.read_json::<Task>(&path).await {
                Ok(t) => t,
                Err(_) => continue,
            };
            if task.run_id != run_id
                || !matches!(task.status(), TaskStatus::Pending | TaskStatus::Paused)
            {
                continue;
            }
            if task
                .depends_on
                .iter()
                .any(|name| failed_task_names.contains(name))
            {
                let updated_task: Task = serde_json::from_value(serde_json::json!({
                    "id": task.id, "run_id": task.run_id, "task_index": task.task_index, "task_name": task.task_name,
                    "executor_type": task.executor_type, "depends_on": task.depends_on, "status": TaskStatus::Failed,
                    "execution_name": task.execution_name, "params": task.params, "output": task.output,
                    "logs": task.logs, "error": error, "dispatched_at": task.dispatched_at,
                    "started_at": task.started_at, "finished_at": Utc::now(), "created_at": task.created_at,
                }))?;
                updated.push(updated_task.task_name.clone());
                self.write_json(&path, &updated_task).await?;
            }
        }
        Ok(updated)
    }

    pub(super) async fn get_run_task_stats_impl(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        let tasks = self.list_tasks_impl(run_id).await?;
        let total = tasks.len() as i64;
        let completed = tasks
            .iter()
            .filter(|t| matches!(t.status(), TaskStatus::Success | TaskStatus::Failed))
            .count() as i64;
        let failed = tasks
            .iter()
            .filter(|t| matches!(t.status(), TaskStatus::Failed))
            .count() as i64;
        Ok((total, completed, failed))
    }

    pub(super) async fn batch_get_run_task_stats_impl(
        &self,
        run_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, (i64, i64, i64)>> {
        let mut result = std::collections::HashMap::new();
        for &run_id in run_ids {
            result.insert(run_id, self.get_run_task_stats_impl(run_id).await?);
        }
        Ok(result)
    }

    pub(super) async fn batch_update_run_statuses_impl(
        &self,
        updates: &[(Uuid, czar_core::models::RunStatus, Option<String>)],
    ) -> Result<()> {
        for (run_id, status, error) in updates {
            self.update_run_status_impl(*run_id, *status, error.as_deref())
                .await?;
        }
        Ok(())
    }
}
