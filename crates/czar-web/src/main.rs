use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use czar_core::database::Database;
use czar_web::api::{ApiServer, bind_api_listener, build_router};
use czar_web::handlers::{reload_workflow_definitions_for_db, resolve_definitions_root};

#[path = "main/bootstrap.rs"]
mod bootstrap;
use bootstrap::{Args, init_database, parse_addr};

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .try_init();
}

fn bootstrap_workflow_definitions_enabled() -> bool {
    std::env::var("CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS")
        .ok()
        .is_some_and(|raw| {
            !matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "" | "0" | "false" | "no" | "off"
            )
        })
}

async fn run_api_server<S>(
    api: ApiServer,
    listener: tokio::net::TcpListener,
    shutdown: S,
) -> anyhow::Result<()>
where
    S: std::future::Future<Output = ()> + Send + 'static,
{
    let api = api.listening_on(listener.local_addr()?);
    let app = build_router(api);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

async fn run_from_args<S>(args: Args, shutdown: S) -> anyhow::Result<()>
where
    S: std::future::Future<Output = ()> + Send + 'static,
{
    let addr: SocketAddr = parse_addr(&args.addr);
    let auth = args.auth.resolve(addr)?;
    let workflows_dir =
        resolve_definitions_root(args.workflows_dir.as_deref()).map_err(anyhow::Error::msg)?;
    // SAFETY: startup sets the process-scoped definitions root before background tasks spawn.
    unsafe {
        std::env::set_var(
            "CZAR_WORKFLOW_DEFINITIONS_DIR",
            workflows_dir.display().to_string(),
        );
    }
    let db: Arc<dyn Database> = init_database(&args).await?;
    let listener = bind_api_listener(addr).await?;

    if bootstrap_workflow_definitions_enabled() {
        let bootstrap_db = Arc::clone(&db);
        let bootstrap_workflows_dir = workflows_dir.clone();
        tokio::spawn(async move {
            match reload_workflow_definitions_for_db(&*bootstrap_db, &bootstrap_workflows_dir).await
            {
                Ok(result) => {
                    if result.reloaded == 0 && result.failed == 0 {
                        eprintln!(
                            "Workflow definition bootstrap discovered {} files in {} but loaded none ({} skipped).",
                            result.discovered, result.definitions_root, result.skipped
                        );
                    } else if result.failed > 0 {
                        eprintln!(
                            "Bootstrapped {} workflow definitions from {} ({} skipped, {} failed).",
                            result.reloaded, result.definitions_root, result.skipped, result.failed
                        );
                    } else {
                        println!(
                            "Bootstrapped {} workflow definitions from {} ({} skipped).",
                            result.reloaded, result.definitions_root, result.skipped
                        );
                    }
                }
                Err(err) => {
                    eprintln!("Failed to bootstrap workflow definitions: {err}");
                }
            }
        });
    }

    println!("Workflow definitions root: {}", workflows_dir.display());
    println!("Serving on http://{}", addr);
    let api = ApiServer::new_with_workflow_definitions_root(db, workflows_dir).with_auth(auth);
    run_api_server(api, listener, shutdown).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();
    run_from_args(args, futures::future::pending::<()>()).await
}

#[cfg(test)]
#[path = "main/tests.rs"]
mod tests;
