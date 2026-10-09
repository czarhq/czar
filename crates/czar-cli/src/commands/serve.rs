use anyhow::{Result, anyhow};
use clap::Args;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use czar_core::config::OrchestratorConfig;
use czar_core::database::Database;
use czar_core::database::tracing::TracingDatabase;
use czar_core::scheduler::Scheduler;
use czar_executors::ExecutorManager;
use czar_web::api::{ApiServer, bind_api_listener, build_router};
use czar_web::auth::AuthArgs;
use czar_web::handlers::{reload_workflow_definitions_for_db, resolve_definitions_root};
use czar_web::ui_bundle::UiBundle;

use crate::bootstrap;
use workers::{load_workers_config, spawn_worker_sync_tasks};

mod workers;

#[derive(Args, Default)]
pub struct Serve {
    /// Optional config file path (YAML)
    #[arg(short, long)]
    pub config: Option<String>,

    /// Address to bind the API server (e.g., 127.0.0.1:4000)
    #[arg(long, default_value = "127.0.0.1:4000")]
    pub addr: String,

    /// Directory to scan for workflow definitions when bootstrapping/reloading
    #[arg(long)]
    pub workflows_dir: Option<String>,

    /// Path to a workers.yaml file listing remote workers to sync from
    #[arg(long)]
    pub workers_file: Option<String>,

    /// Extra browser origin allowed to call the API, e.g. https://czar.example.com
    /// (repeatable). Loopback and same-origin requests are always allowed.
    #[arg(long = "allowed-origin", value_name = "ORIGIN", value_parser = czar_web::request_guard::parse_allowed_origin)]
    pub allowed_origins: Vec<String>,

    #[command(flatten)]
    pub auth: AuthArgs,
}

impl Serve {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        if std::env::var("CZAR_DB_TRACE").is_ok() {
            let traced = Arc::new(TracingDatabase::new(db));
            return self.execute_inner(traced).await;
        }
        self.execute_inner(db).await
    }

    async fn execute_inner<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let addr: SocketAddr = self.addr.parse()?;
        let auth = self.auth.resolve(addr)?;
        let workers_file = self.workers_file.as_deref().map(|s| s.to_string());
        let workflows_dir = resolve_definitions_root(self.workflows_dir.as_deref())
            .map_err(|message| anyhow!(message))?;
        // SAFETY: startup sets the process-scoped definitions root before background tasks spawn.
        unsafe {
            std::env::set_var(
                "CZAR_WORKFLOW_DEFINITIONS_DIR",
                workflows_dir.display().to_string(),
            );
        }

        db.run_migrations().await?;

        info!("Starting orchestrator...");
        let job_trackers = crate::commands::cloud::job_trackers().await?;
        let project_notifications = bootstrap::project_notifications_from(&workflows_dir)?;
        let mut orchestrator_config = match self.config {
            Some(config_path) => {
                let config_content = std::fs::read_to_string(&config_path)?;
                serde_yaml::from_str(&config_content)?
            }
            None => OrchestratorConfig::default(),
        };
        if orchestrator_config.notifications.is_empty() {
            orchestrator_config.notifications = project_notifications;
        }
        let all_workers =
            load_workers_config(workers_file.as_deref(), &orchestrator_config.workers);
        let executor_manager = Arc::new(ExecutorManager::new().with_workers(&all_workers));
        let scheduler =
            Scheduler::new_with_config(db.clone(), executor_manager, orchestrator_config)
                .with_job_trackers(job_trackers);

        let listener = bind_api_listener(addr).await?;
        println!("Workflow definitions root: {}", workflows_dir.display());

        if bootstrap_workflow_definitions_enabled() {
            spawn_bootstrap_task(db.clone(), workflows_dir.clone());
        }

        spawn_worker_sync_tasks(&db, &all_workers);

        let allowed_origins = self.allowed_origins;
        let web_db = db.clone();
        let web_workflows_dir: PathBuf = workflows_dir;
        let web_handle = tokio::spawn(async move {
            let api = ApiServer::new_with_workflow_definitions_root(web_db, web_workflows_dir)
                .listening_on(addr)
                .with_allowed_origins(allowed_origins)
                .with_auth(auth);
            let app = build_router(api);
            match UiBundle::resolve() {
                Ok(bundle) => {
                    println!(
                        "API + React UI server available at http://{} (bundle: {})",
                        addr,
                        bundle.dist_dir().display()
                    );
                }
                Err(err) => {
                    eprintln!("{}", err.missing_message());
                    println!("API server available at http://{}", addr);
                }
            }
            axum::serve(listener, app).await?;
            Ok::<(), anyhow::Error>(())
        });

        let scheduler_result = scheduler.run().await;
        web_handle.abort();
        scheduler_result
    }
}

fn spawn_bootstrap_task<D: Database + 'static>(db: Arc<D>, workflows_dir: PathBuf) {
    tokio::spawn(async move {
        match reload_workflow_definitions_for_db(&*db, &workflows_dir).await {
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
            Err(err) => eprintln!("Failed to bootstrap workflow definitions: {err}"),
        }
    });
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

#[cfg(test)]
mod tests;
