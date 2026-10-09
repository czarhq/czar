use clap::Parser;
use std::future::Future;
use tracing::{info, warn};

mod auth;
mod execute;
mod handlers;
mod routes;

pub use handlers::health_handler;

#[derive(Parser, Debug, Default)]
#[command(name = "czar-worker")]
#[command(about = "Czar Worker - Stateless task execution server")]
struct Args {
    /// Address to bind to
    #[arg(long, default_value = "127.0.0.1:8081")]
    addr: String,

    /// Path to workflow YAML (embedded in container)
    #[arg(long, default_value = "workflow.yaml")]
    workflow_path: String,

    /// Working directory for task execution
    #[arg(long, default_value = ".")]
    working_dir: String,

    /// Require `Authorization: Bearer <TOKEN>` on /compile and /execute
    #[arg(long, env = "CZAR_WORKER_AUTH_TOKEN", hide_env_values = true)]
    auth_token: Option<String>,
}

#[derive(Clone, Default)]
pub struct WorkerState {
    pub workflow_path: String,
    pub working_dir: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();
    run_from_args(args, std::future::pending::<()>()).await
}

async fn run_from_args<S>(args: Args, shutdown: S) -> anyhow::Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    info!("Starting Czar Worker");
    info!("  Address: {}", args.addr);
    info!("  Workflow: {}", args.workflow_path);
    info!("  Working directory: {}", args.working_dir);

    let state = WorkerState {
        workflow_path: args.workflow_path,
        working_dir: args.working_dir,
    };

    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    info!("Worker listening on {}", args.addr);
    if args.auth_token.is_none() && !listener.local_addr()?.ip().is_loopback() {
        warn!(
            "Worker listens on non-loopback address {} without --auth-token: anyone who can reach it can run arbitrary commands",
            args.addr
        );
    }

    run_with_shutdown(listener, state, args.auth_token, shutdown).await
}

async fn run_with_shutdown<S>(
    listener: tokio::net::TcpListener,
    state: WorkerState,
    auth_token: Option<String>,
    shutdown: S,
) -> anyhow::Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    let app = routes::build_app(state, auth_token);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt::try_init();
}

#[cfg(test)]
#[path = "main/tests/mod.rs"]
mod tests;
