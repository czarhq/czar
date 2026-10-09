use clap::Subcommand;

use super::{
    CreateWorkflowYaml, DeleteWorkflow, Dev, Init, ListWorkflows, Logs, Run, Schema, Serve, Status,
    Tasks, Trigger, ValidateWorkflow, WorkflowCommand,
};

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize the database with migrations
    #[command(display_order = 0)]
    Init(Init),

    /// Start the scheduler and web UI server
    #[command(display_order = 1)]
    Serve(Serve),

    /// Start the local development stack (alias for serve)
    #[command(display_order = 2)]
    Dev(Dev),

    /// Run a workflow YAML locally (apply + trigger + wait for completion)
    #[command(display_order = 3)]
    Run(Run),

    /// Workflow management commands
    #[command(name = "workflow", display_order = 4)]
    Workflow(WorkflowCommand),

    /// Trigger an existing workflow run (supports flow + selectors)
    #[command(display_order = 5)]
    Trigger(Trigger),

    /// Load a workflow YAML file into the database
    #[command(name = "load", display_order = 6)]
    CreateWorkflowYaml(CreateWorkflowYaml),

    /// List all workflows
    #[command(display_order = 7)]
    ListWorkflows(ListWorkflows),

    /// Delete a workflow (and all associated runs/tasks)
    #[command(display_order = 8)]
    DeleteWorkflow(DeleteWorkflow),

    /// Show status of runs
    #[command(display_order = 9)]
    Status(Status),

    /// Show tasks for a run
    #[command(display_order = 10)]
    Tasks(Tasks),

    /// Show task logs for a run
    #[command(display_order = 11)]
    Logs(Logs),

    /// Schema inspection commands
    #[command(name = "schema", display_order = 12)]
    Schema(Schema),

    /// Validate a workflow YAML file against the strict workflow schema
    #[command(display_order = 13)]
    ValidateWorkflow(ValidateWorkflow),
}

impl Default for Commands {
    fn default() -> Self {
        Self::Init(Init)
    }
}

impl Commands {
    pub fn uses_api(&self) -> bool {
        matches!(self, Commands::ValidateWorkflow(_) | Commands::Schema(_))
    }

    /// Commands that write to the database and don't migrate on their own
    /// (init, serve and dev do). Read-only commands never change the schema.
    pub fn needs_migrations(&self) -> bool {
        matches!(
            self,
            Commands::Run(_)
                | Commands::Workflow(_)
                | Commands::Trigger(_)
                | Commands::CreateWorkflowYaml(_)
                | Commands::DeleteWorkflow(_)
        )
    }
}
