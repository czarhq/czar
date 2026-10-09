use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::path::PathBuf;

use czar_core::{compiled::inspect_python_task_signature, resolve_python_execution_root};

#[derive(Args)]
pub struct Schema {
    #[command(subcommand)]
    pub action: SchemaAction,
}

impl Default for Schema {
    fn default() -> Self {
        Self {
            action: SchemaAction::Export(SchemaExport::default()),
        }
    }
}

#[derive(Subcommand)]
pub enum SchemaAction {
    /// Export the inferred input/output schema for a Python task file
    Export(SchemaExport),
}

impl Schema {
    pub async fn execute(self) -> Result<()> {
        match self.action {
            SchemaAction::Export(cmd) => cmd.execute().await,
        }
    }
}

#[derive(Args, Default)]
pub struct SchemaExport {
    /// Python task file to inspect
    #[arg(value_name = "FILE")]
    pub file: String,

    /// Python function name to inspect
    #[arg(long, default_value = "main")]
    pub function: String,

    /// Explicit Python project root containing pyproject.toml
    #[arg(long)]
    pub project_root: Option<String>,
}

impl SchemaExport {
    pub async fn execute(self) -> Result<()> {
        let file = PathBuf::from(&self.file)
            .canonicalize()
            .with_context(|| format!("failed to resolve task file {}", self.file))?;
        let project_root = self.project_root.as_deref().map(PathBuf::from);
        let resolved_project_root = resolve_python_execution_root(
            project_root.as_deref(),
            Some(&file),
            None,
            "schema export",
        )
        .map_err(anyhow::Error::msg)?;
        let signature =
            inspect_python_task_signature(&file, &self.function, &resolved_project_root)
                .map_err(anyhow::Error::msg)?;
        let yaml =
            serde_yaml::to_string(&signature).context("failed to serialize inferred schema")?;
        print!("{yaml}");
        Ok(())
    }
}
