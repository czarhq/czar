use anyhow::{Result, anyhow};
use clap::Args;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

use czar_core::database::Database;
use uuid::Uuid;

#[derive(Args, Default)]
pub struct Logs {
    /// Run ID
    pub run_id: String,

    /// Optional task name or task ID to inspect. Omit to print logs for all tasks in the run.
    pub task: Option<String>,

    /// Follow logs until the run finishes
    #[arg(long)]
    pub follow: bool,

    #[arg(long, hide = true, default_value_t = 1000)]
    pub poll_interval_ms: u64,
}

impl Logs {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        if self.follow {
            return self.execute_follow(db).await;
        }

        self.execute_once(db).await
    }

    async fn execute_once(self, db: Arc<dyn Database>) -> Result<()> {
        let run_id = self.run_id.parse()?;
        let tasks = db.list_tasks(run_id).await?;

        if tasks.is_empty() {
            println!("No tasks found for run {}", run_id);
            return Ok(());
        }

        let selected: Vec<_> = if let Some(task_selector) = self.task.as_deref() {
            let matches: Vec<_> = tasks
                .into_iter()
                .filter(|task| {
                    task.task_name == task_selector || task.id.to_string() == task_selector
                })
                .collect();

            if matches.is_empty() {
                return Err(anyhow!(
                    "No task named or identified by '{}' found in run {}",
                    task_selector,
                    run_id
                ));
            }

            matches
        } else {
            tasks
        };

        for (index, task) in selected.iter().enumerate() {
            if index > 0 {
                println!();
            }

            println!(
                "== {} ({}, {}) ==",
                task.task_name,
                task.id,
                task.status_str()
            );

            let mut printed_body = false;
            if let Some(logs) = task.logs.as_deref()
                && !logs.trim().is_empty()
            {
                print!("{logs}");
                if !logs.ends_with('\n') {
                    println!();
                }
                printed_body = true;
            }

            if let Some(error) = task.error.as_deref()
                && !error.trim().is_empty()
            {
                if printed_body {
                    println!();
                }
                println!("Error:");
                println!("{error}");
                printed_body = true;
            }

            if !printed_body {
                println!("(no logs or error recorded)");
            }
        }

        Ok(())
    }

    async fn execute_follow(self, db: Arc<dyn Database>) -> Result<()> {
        let run_id: Uuid = self.run_id.parse()?;
        let mut printed_headers = HashSet::new();
        let mut printed_log_lengths: HashMap<Uuid, usize> = HashMap::new();
        let mut printed_errors: HashMap<Uuid, String> = HashMap::new();

        loop {
            let tasks = db.list_tasks(run_id).await?;
            if tasks.is_empty() {
                println!("No tasks found for run {}", run_id);
                return Ok(());
            }

            let selected: Vec<_> = if let Some(task_selector) = self.task.as_deref() {
                let matches: Vec<_> = tasks
                    .into_iter()
                    .filter(|task| {
                        task.task_name == task_selector || task.id.to_string() == task_selector
                    })
                    .collect();

                if matches.is_empty() {
                    return Err(anyhow!(
                        "No task named or identified by '{}' found in run {}",
                        task_selector,
                        run_id
                    ));
                }

                matches
            } else {
                tasks
            };

            for task in &selected {
                if printed_headers.insert(task.id) {
                    println!(
                        "== {} ({}, {}) ==",
                        task.task_name,
                        task.id,
                        task.status_str()
                    );
                }

                let printed_len = printed_log_lengths.get(&task.id).copied().unwrap_or(0);
                if let Some(logs) = task.logs.as_deref()
                    && logs.len() > printed_len
                    && let Some(new_chunk) = logs.get(printed_len..)
                {
                    print!("{new_chunk}");
                    if !new_chunk.ends_with('\n') {
                        println!();
                    }
                    printed_log_lengths.insert(task.id, logs.len());
                }

                if let Some(error) = task.error.as_deref()
                    && !error.trim().is_empty()
                {
                    let already_printed = printed_errors.get(&task.id);
                    if already_printed != Some(&error.to_string()) {
                        println!("Error:");
                        println!("{error}");
                        printed_errors.insert(task.id, error.to_string());
                    }
                }
            }

            let run = db.get_run(run_id).await?;
            let all_selected_terminal = selected.iter().all(|task| task.status().is_terminal());
            if run.status().is_terminal() && all_selected_terminal {
                return Ok(());
            }

            sleep(Duration::from_millis(self.poll_interval_ms)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use czar_core::database::{NewTask, RunRepository, TaskRepository, WorkflowRepository};
    use czar_core::models::TaskStatus;
    use czar_store::SqliteDatabase;

    #[tokio::test]
    async fn test_logs_command_handles_empty_and_selected_tasks() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");
        let workflow = db
            .create_workflow(
                "wf-logs", None, "job", "local", "local", "process", None, None,
            )
            .await
            .expect("create workflow");
        let run = db
            .create_run(workflow.id, "test")
            .await
            .expect("create run");

        Logs {
            run_id: run.id.to_string(),
            task: None,
            follow: false,
            poll_interval_ms: 1000,
        }
        .execute(db.clone())
        .await
        .expect("logs command should handle empty task list");

        db.batch_create_dag_tasks(
            run.id,
            &[NewTask {
                task_index: 0,
                task_name: "task-a".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo hi"}),
                max_retries: 0,
                timeout_seconds: Some(10),
            }],
        )
        .await
        .expect("create task");

        let task = db
            .list_tasks(run.id)
            .await
            .expect("list tasks")
            .into_iter()
            .next()
            .expect("task exists");
        db.append_task_log(task.id, "hello from logs\n")
            .await
            .expect("append log");
        db.update_task_status(task.id, TaskStatus::Failed, None, Some("boom"))
            .await
            .expect("update status");

        Logs {
            run_id: run.id.to_string(),
            task: Some("task-a".to_string()),
            follow: false,
            poll_interval_ms: 1000,
        }
        .execute(db.clone())
        .await
        .expect("logs command should handle task name selection");

        Logs {
            run_id: run.id.to_string(),
            task: Some(task.id.to_string()),
            follow: false,
            poll_interval_ms: 1000,
        }
        .execute(db)
        .await
        .expect("logs command should handle task id selection");
    }

    #[tokio::test]
    async fn test_logs_command_follow_exits_once_run_and_tasks_are_terminal() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");
        let workflow = db
            .create_workflow(
                "wf-logs-follow",
                None,
                "job",
                "local",
                "local",
                "process",
                None,
                None,
            )
            .await
            .expect("create workflow");
        let run = db
            .create_run(workflow.id, "test")
            .await
            .expect("create run");
        db.batch_create_dag_tasks(
            run.id,
            &[NewTask {
                task_index: 0,
                task_name: "task-a".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo hi"}),
                max_retries: 0,
                timeout_seconds: Some(10),
            }],
        )
        .await
        .expect("create task");
        let task = db
            .list_tasks(run.id)
            .await
            .expect("list tasks")
            .into_iter()
            .next()
            .expect("task exists");
        db.append_task_log(task.id, "follow me\n")
            .await
            .expect("append log");
        db.update_task_status(task.id, TaskStatus::Success, None, None)
            .await
            .expect("update task status");
        db.update_run_status(run.id, czar_core::models::RunStatus::Success, None)
            .await
            .expect("update run status");

        Logs {
            run_id: run.id.to_string(),
            task: None,
            follow: true,
            poll_interval_ms: 1,
        }
        .execute(db)
        .await
        .expect("follow should exit for terminal runs");
    }
}
