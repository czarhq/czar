mod bootstrap;
mod commands;

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use bootstrap::{DatabaseArgs, DatabaseBackend, resolve_backend};
use commands::Commands;
use czar_core::database::Database;

#[derive(Parser, Default)]
#[command(name = "czar")]
#[command(about = "Czar - A high-performance task orchestrator supporting multiple execution backends", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[command(flatten)]
    database: DatabaseArgs,
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "error".into());
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_ansi(false)
        .finish();
    if tracing::subscriber::set_global_default(subscriber).is_err() {
        // Tests may initialize tracing multiple times; it's fine once a global
        // subscriber is already installed.
        return Ok(());
    }
    Ok(())
}

async fn handle_api_command(command: Commands) -> Result<Option<Commands>> {
    if command.uses_api() {
        match command {
            Commands::ValidateWorkflow(cmd) => {
                cmd.execute().await?;
                return Ok(None);
            }
            Commands::Schema(cmd) => {
                cmd.execute().await?;
                return Ok(None);
            }
            _ => {}
        }
    }
    Ok(Some(command))
}

async fn dispatch_command<D>(command: Commands, db: Arc<D>) -> Result<()>
where
    D: Database + 'static,
{
    if command.needs_migrations() {
        db.run_migrations().await?;
    }
    match command {
        Commands::Init(cmd) => cmd.execute(db).await?,
        Commands::Serve(cmd) => cmd.execute(db).await?,
        Commands::Dev(cmd) => cmd.execute(db).await?,
        Commands::Run(cmd) => cmd.execute(db).await?,
        Commands::Workflow(cmd) => cmd.execute(db).await?,
        Commands::CreateWorkflowYaml(cmd) => cmd.execute(db).await?,
        Commands::ListWorkflows(cmd) => cmd.execute(db).await?,
        Commands::DeleteWorkflow(cmd) => cmd.execute(db).await?,
        Commands::Trigger(cmd) => cmd.execute(db).await?,
        Commands::Status(cmd) => cmd.execute(db).await?,
        Commands::Tasks(cmd) => cmd.execute(db).await?,
        Commands::Logs(cmd) => cmd.execute(db).await?,
        Commands::Schema(_) | Commands::ValidateWorkflow(_) => {
            unreachable!("pre-db command handled earlier")
        }
    }
    Ok(())
}

async fn run_cli(cli: Cli) -> Result<()> {
    let Cli { command, database } = cli;

    let Some(command) = handle_api_command(command).await? else {
        return Ok(());
    };

    match resolve_backend(database.db_backend, database.database_url.as_deref())? {
        DatabaseBackend::Postgres => {
            #[cfg(feature = "postgres")]
            {
                let db = bootstrap::init_postgres_database(&database).await?;
                dispatch_command(command, db).await
            }

            #[cfg(not(feature = "postgres"))]
            {
                unreachable!("resolve_backend should reject unavailable postgres backend")
            }
        }
        DatabaseBackend::Sqlite => {
            #[cfg(feature = "sqlite")]
            {
                let db = bootstrap::init_sqlite_database(&database).await?;
                dispatch_command(command, db).await
            }

            #[cfg(not(feature = "sqlite"))]
            {
                unreachable!("resolve_backend should reject unavailable sqlite backend")
            }
        }
        DatabaseBackend::D1 => {
            #[cfg(feature = "d1")]
            {
                let db = bootstrap::init_d1_database(&database).await?;
                dispatch_command(command, db).await
            }

            #[cfg(not(feature = "d1"))]
            {
                unreachable!("resolve_backend should reject unavailable d1 backend")
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing()?;
    let cli = Cli::parse();
    let _ = bootstrap::load_project_env_from_current_dir()?;
    run_cli(cli).await
}

#[cfg(test)]
#[path = "main/tests/mod.rs"]
mod tests;
