use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{DeleteWorkflow, ListWorkflows};
use crate::bootstrap;
use czar_core::database::Database;
use czar_core::models::Workflow;
use czar_core::services::workflow_yaml::{
    ApplyWorkflowYamlOutcome, ApplyWorkflowYamlRequest, WorkflowServiceError, apply_workflow_yaml,
};
use czar_core::workflow::{ValidationOptions, WorkflowDefinition};
use czar_core::workflow_source::load_workflow_source;

#[derive(Args)]
pub struct WorkflowCommand {
    #[command(subcommand)]
    pub action: WorkflowAction,
}

impl Default for WorkflowCommand {
    fn default() -> Self {
        Self {
            action: WorkflowAction::List(ListWorkflows),
        }
    }
}

#[derive(Subcommand)]
pub enum WorkflowAction {
    /// Deploy or replace a workflow definition from YAML
    Deploy(WorkflowDeploy),

    /// List workflows
    List(ListWorkflows),

    /// Show workflow metadata and recent runs
    Status(WorkflowStatus),

    /// Delete a workflow
    Delete(DeleteWorkflow),

    /// Update workflow metadata without replacing the workflow definition
    Update(WorkflowUpdate),
}

impl WorkflowCommand {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        match self.action {
            WorkflowAction::Deploy(cmd) => cmd.execute(db).await,
            WorkflowAction::List(cmd) => cmd.execute(db).await,
            WorkflowAction::Status(cmd) => cmd.execute(db).await,
            WorkflowAction::Delete(cmd) => cmd.execute(db).await,
            WorkflowAction::Update(cmd) => cmd.execute(db).await,
        }
    }
}

#[derive(Args, Default)]
pub struct WorkflowDeploy {
    /// Path to workflow YAML file
    #[arg(value_name = "FILE")]
    pub file: String,

    /// Project label for the workflow (default: local)
    #[arg(short, long, default_value = "local")]
    pub project: String,

    /// Region label for the workflow (default: local)
    #[arg(short, long, default_value = "local")]
    pub region: String,
}

impl WorkflowDeploy {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let yaml_content = load_workflow_source(Path::new(&self.file))?;
        let root = Path::new(&self.file)
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let root = root.canonicalize().unwrap_or(root);
        let validation_options = if bootstrap::project_contract_validation_enabled_from(&root)? {
            ValidationOptions::STRICT
        } else {
            ValidationOptions::RELAXED
        };
        let definition = WorkflowDefinition::parse_yaml_with_root(&yaml_content, &root)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        let workflow_name = definition.name.clone();
        let result = apply_workflow_yaml(
            &*db,
            ApplyWorkflowYamlRequest::new(&yaml_content, &root, &self.project, &self.region)
                .with_persist_schedule_on_create(true)
                .with_validation_options(validation_options),
        )
        .await
        .map_err(map_workflow_service_error)?;

        let schedule_enabled = definition.schedule.is_some();
        db.update_workflow_schedule(
            result.workflow.id,
            definition.schedule.as_deref(),
            schedule_enabled,
        )
        .await?;
        let workflow = db.get_workflow(&workflow_name).await?;

        let action = match result.outcome {
            ApplyWorkflowYamlOutcome::Created => "Deployed",
            ApplyWorkflowYamlOutcome::Replaced => "Updated",
        };
        print_workflow_summary(action, &workflow);
        Ok(())
    }
}

#[derive(Args, Default)]
pub struct WorkflowStatus {
    /// Workflow name
    pub workflow_name: String,
}

impl WorkflowStatus {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let workflow = db.get_workflow(&self.workflow_name).await?;
        println!("Workflow: {}", workflow.name);
        println!("ID: {}", workflow.id);
        println!("Executor: {}", workflow.executor_type);
        println!("Project: {}", workflow.project);
        println!("Region: {}", workflow.region);
        println!(
            "Schedule: {}",
            workflow.schedule.as_deref().unwrap_or("(not configured)")
        );
        println!("Schedule enabled: {}", workflow.schedule_enabled);
        println!(
            "Current snapshot: {}",
            workflow
                .current_snapshot_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "-".to_string())
        );

        let runs = db.list_runs(Some(workflow.id)).await?;
        if runs.is_empty() {
            println!();
            println!("Recent runs: none");
            return Ok(());
        }

        println!();
        println!("Recent runs:");
        println!(
            "{:<36} {:<12} {:<20} {:<20}",
            "Run ID", "Status", "Started", "Finished"
        );
        println!("{}", "-".repeat(92));

        for run in runs.into_iter().take(10) {
            let started = run
                .started_at
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "-".to_string());
            let finished = run
                .finished_at
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "-".to_string());
            println!(
                "{:<36} {:<12} {:<20} {:<20}",
                run.id,
                run.status_str(),
                started,
                finished
            );
        }

        Ok(())
    }
}

#[derive(Args, Default)]
pub struct WorkflowUpdate {
    /// Workflow name
    pub workflow_name: String,

    /// Enable or disable the configured schedule
    #[arg(long, value_name = "BOOL")]
    pub schedule_enabled: Option<bool>,
}

impl WorkflowUpdate {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let workflow = db.get_workflow(&self.workflow_name).await?;
        let Some(schedule_enabled) = self.schedule_enabled else {
            return Err(anyhow::anyhow!(
                "no workflow updates requested; pass at least one flag such as --schedule-enabled"
            ));
        };

        if schedule_enabled && workflow.schedule.is_none() {
            return Err(anyhow::anyhow!(
                "workflow '{}' has no schedule configured, so it cannot be enabled",
                workflow.name
            ));
        }

        db.update_workflow_schedule(workflow.id, workflow.schedule.as_deref(), schedule_enabled)
            .await?;
        let updated = db.get_workflow(&self.workflow_name).await?;
        print_workflow_summary("Updated", &updated);
        println!("  Schedule enabled: {}", updated.schedule_enabled);
        Ok(())
    }
}

fn print_workflow_summary(action: &str, workflow: &Workflow) {
    println!("✓ {} workflow: {}", action, workflow.name);
    println!("  ID: {}", workflow.id);
    println!("  Executor: {}", workflow.executor_type);
    println!("  Project: {}", workflow.project);
    println!("  Region: {}", workflow.region);
    if let Some(schedule) = workflow.schedule.as_deref() {
        println!("  Schedule: {}", schedule);
    }
}

fn map_workflow_service_error(err: WorkflowServiceError) -> anyhow::Error {
    match err {
        WorkflowServiceError::WorkflowAlreadyExists { name } => {
            anyhow::anyhow!("workflow already exists: {}", name)
        }
        WorkflowServiceError::InvalidYaml(err) => anyhow::Error::new(err),
        WorkflowServiceError::InvalidWorkflow(msg) | WorkflowServiceError::Compilation(msg) => {
            anyhow::anyhow!(msg)
        }
        WorkflowServiceError::Storage(err) => err,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use czar_core::database::WorkflowRepository;
    use czar_store::SqliteDatabase;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_workflow_update_toggles_schedule_enabled() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");
        db.create_workflow(
            "wf-update",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            Some("*/5 * * * *"),
        )
        .await
        .expect("create workflow");

        WorkflowUpdate {
            workflow_name: "wf-update".to_string(),
            schedule_enabled: Some(true),
        }
        .execute(db.clone())
        .await
        .expect("enable schedule");

        let workflow = db.get_workflow("wf-update").await.expect("get workflow");
        assert!(workflow.schedule_enabled);

        WorkflowUpdate {
            workflow_name: "wf-update".to_string(),
            schedule_enabled: Some(false),
        }
        .execute(db.clone())
        .await
        .expect("disable schedule");

        let workflow = db.get_workflow("wf-update").await.expect("get workflow");
        assert!(!workflow.schedule_enabled);
    }

    #[tokio::test]
    async fn test_workflow_update_rejects_enabling_missing_schedule() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");
        db.create_workflow(
            "wf-no-schedule",
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

        let err = WorkflowUpdate {
            workflow_name: "wf-no-schedule".to_string(),
            schedule_enabled: Some(true),
        }
        .execute(db)
        .await
        .expect_err("enabling schedule without cron should fail");

        assert!(err.to_string().contains("has no schedule configured"));
    }
}
