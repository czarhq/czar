use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use crate::bootstrap;
use czar_core::database::Database;
use czar_core::services::workflow_yaml::{
    ApplyWorkflowYamlRequest, ExistingWorkflowBehavior, WorkflowServiceError, apply_workflow_yaml,
};
use czar_core::workflow::ValidationOptions;
use czar_core::workflow_source::load_workflow_source;

#[derive(Args, Default)]
pub struct CreateWorkflowYaml {
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

impl CreateWorkflowYaml {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        let yaml_content = load_workflow_source(std::path::Path::new(&self.file))?;
        let workflow = create_workflow_from_yaml_str(
            &*db,
            &yaml_content,
            &self.file,
            &self.project,
            &self.region,
        )
        .await?;

        println!("✓ Created workflow from YAML: {}", workflow.name);
        println!("  ID: {}", workflow.id);
        println!("  Executor: {}", workflow.executor_type);
        println!("  Project: {}", workflow.project);
        println!("  Region: {}", workflow.region);
        Ok(())
    }
}

pub async fn create_workflow_from_yaml_str(
    db: &dyn Database,
    yaml_content: &str,
    file_path: &str,
    project: &str,
    region: &str,
) -> Result<czar_core::models::Workflow> {
    let root = std::path::Path::new(file_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
    let root = root.canonicalize().unwrap_or(root);
    let validation_options = if bootstrap::project_contract_validation_enabled_from(&root)? {
        ValidationOptions::STRICT
    } else {
        ValidationOptions::RELAXED
    };

    let result = apply_workflow_yaml(
        db,
        ApplyWorkflowYamlRequest::new(yaml_content, &root, project, region)
            .with_existing_workflow(ExistingWorkflowBehavior::Error)
            .with_validation_options(validation_options),
    )
    .await
    .map_err(|err| match err {
        WorkflowServiceError::WorkflowAlreadyExists { name } => {
            anyhow::anyhow!("workflow already exists: {}", name)
        }
        WorkflowServiceError::InvalidYaml(err) => anyhow::Error::new(err),
        WorkflowServiceError::InvalidWorkflow(msg) | WorkflowServiceError::Compilation(msg) => {
            anyhow::anyhow!(msg)
        }
        WorkflowServiceError::Storage(err) => err,
    })?;

    Ok(result.workflow)
}

#[cfg(test)]
mod tests;
