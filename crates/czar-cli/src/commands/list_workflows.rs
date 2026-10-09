use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use czar_core::database::Database;

#[derive(Args, Default)]
pub struct ListWorkflows;

impl ListWorkflows {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        let workflows = db.list_workflows().await?;

        if workflows.is_empty() {
            println!("No workflows found");
        } else {
            println!("Workflows:");
            println!("{:<36} {:<30} {:<30}", "ID", "Name", "Cloud Run Job");
            println!("{}", "-".repeat(96));

            for wf in workflows {
                println!("{:<36} {:<30} {:<30}", wf.id, wf.name, wf.job_name);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
