use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use czar_core::database::Database;
use czar_core::models::Run;

#[derive(Args, Default)]
pub struct Status {
    /// Specific run ID to show (optional)
    pub run_id: Option<String>,

    /// Workflow name to filter by (optional)
    #[arg(short, long)]
    pub workflow: Option<String>,
}

impl Status {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        let runs = if let Some(rid) = self.run_id {
            let run_uuid = rid.parse()?;
            vec![db.get_run(run_uuid).await?]
        } else if let Some(wf_name) = self.workflow {
            let workflow = db.get_workflow(&wf_name).await?;
            db.list_runs(Some(workflow.id)).await?
        } else {
            db.list_runs(None).await?
        };

        if runs.is_empty() {
            println!("No runs found");
        } else {
            print_runs(&runs);
        }

        Ok(())
    }
}

pub(crate) fn print_runs(runs: &[Run]) {
    println!("Runs:");
    println!(
        "{:<36} {:<36} {:<12} {:<20} {:<20}",
        "Run ID", "Workflow ID", "Status", "Started", "Finished"
    );
    println!("{}", "-".repeat(124));

    for run in runs {
        let started = run
            .started_at
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "-".to_string());

        let finished = run
            .finished_at
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "-".to_string());

        println!(
            "{:<36} {:<36} {:<12} {:<20} {:<20}",
            run.id,
            run.workflow_id,
            run.status_str(),
            started,
            finished
        );
    }
}

#[cfg(test)]
mod tests;
