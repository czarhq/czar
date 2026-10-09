use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use czar_core::database::Database;

#[derive(Args, Default)]
pub struct Tasks {
    /// Run ID
    pub run_id: String,
}

impl Tasks {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        let run_id = self.run_id.parse()?;
        let tasks = db.list_tasks(run_id).await?;

        if tasks.is_empty() {
            println!("No tasks found for run {}", run_id);
        } else {
            println!("Tasks for run {}:", run_id);
            println!(
                "{:<5} {:<24} {:<36} {:<12} {:<40} {:<20}",
                "Index", "Name", "Task ID", "Status", "Execution", "Finished"
            );
            println!("{}", "-".repeat(149));

            for task in tasks {
                let execution = task.execution_name.as_deref().unwrap_or("-");

                let finished = task
                    .finished_at
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "-".to_string());

                println!(
                    "{:<5} {:<24} {:<36} {:<12} {:<40} {:<20}",
                    task.task_index,
                    task.task_name,
                    task.id,
                    task.status_str(),
                    execution,
                    finished
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
