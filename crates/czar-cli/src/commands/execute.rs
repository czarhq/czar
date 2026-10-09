use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{Duration, timeout};
use tracing::info;

use czar_core::config::OrchestratorConfig;
use czar_core::database::Database;
use czar_core::models::{RunStatus, TaskStatus};
use czar_core::scheduler::Scheduler;
use czar_core::workflow::ValidationOptions;
use czar_core::workflow_source::load_workflow_source;
use czar_executors::ExecutorManager;

use crate::bootstrap;
use crate::commands::selection_spec::FlowSelectionArgs;
use crate::commands::trigger::{build_flow_selection, build_run_launch_json};

#[derive(Args)]
pub struct Execute {
    /// Path to workflow YAML file
    #[arg(short, long)]
    pub file: String,

    /// Optional config file path (YAML)
    #[arg(short, long)]
    pub config: Option<String>,

    #[command(flatten)]
    pub selection: FlowSelectionArgs,

    /// Maximum time to wait for completion in seconds
    #[arg(short, long, default_value = "300")]
    pub timeout: u64,

    /// Override workflow vars from the CLI (repeatable: --var NAME=VALUE)
    #[arg(long = "var", value_name = "NAME=VALUE", value_parser = super::run::parse_var_override)]
    pub vars: Vec<(String, String)>,

    /// Suppress live task log output while the run is executing
    #[arg(long, conflicts_with = "verbose")]
    pub silent: bool,

    /// Show task lifecycle updates in addition to live task logs
    #[arg(long, conflicts_with = "silent")]
    pub verbose: bool,
}

impl Default for Execute {
    fn default() -> Self {
        Self {
            file: String::new(),
            config: None,
            selection: FlowSelectionArgs::default(),
            timeout: 300,
            vars: Vec::new(),
            silent: false,
            verbose: false,
        }
    }
}

#[derive(Default)]
struct RunLogStreamer {
    printed_log_bytes: HashMap<uuid::Uuid, usize>,
    last_statuses: HashMap<uuid::Uuid, TaskStatus>,
    verbose: bool,
}

impl RunLogStreamer {
    fn new(verbose: bool) -> Self {
        Self {
            printed_log_bytes: HashMap::new(),
            last_statuses: HashMap::new(),
            verbose,
        }
    }

    fn emit(&mut self, tasks: &[czar_core::models::Task]) {
        for task in tasks {
            if self.verbose {
                let previous = self.last_statuses.insert(task.id, task.status());
                if previous != Some(task.status()) && !matches!(task.status(), TaskStatus::Pending)
                {
                    println!("[{}] {}", task.task_name, task.status_str());
                }
            }

            let Some(logs) = task.logs.as_deref() else {
                continue;
            };
            let printed = self.printed_log_bytes.entry(task.id).or_insert(0);
            if *printed > logs.len() {
                *printed = 0;
            }
            if let Some(chunk) = logs.get(*printed..) {
                for line in chunk.lines() {
                    if line.is_empty() {
                        println!("[{}]", task.task_name);
                    } else {
                        println!("[{}] {}", task.task_name, line);
                    }
                }
            }
            *printed = logs.len();
        }
    }
}

impl Execute {
    async fn should_start_local_scheduler(db: &dyn Database, run_id: uuid::Uuid) -> Result<bool> {
        // If another scheduler picks this run up quickly, avoid starting a second one.
        for _ in 0..10 {
            let run = db.get_run(run_id).await?;
            if !matches!(run.status(), RunStatus::Pending) {
                return Ok(false);
            }
            let tasks = db.list_tasks(run_id).await?;
            if !tasks.is_empty() {
                return Ok(false);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(true)
    }

    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let start_time = Instant::now();

        // Read and create workflow from YAML
        info!("Creating workflow from {}", self.file);
        let yaml_content = load_workflow_source(std::path::Path::new(&self.file))
            .with_context(|| format!("Failed to load workflow source: {}", self.file))?;

        // Always apply the workflow YAML (creates or updates with snapshots)
        let root = std::path::Path::new(&self.file)
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
        let project_notifications = bootstrap::project_notifications_from(&root)?;

        let var_overrides: BTreeMap<String, String> = self.vars.into_iter().collect();

        let result = czar_core::services::workflow_yaml::apply_workflow_yaml(
            &*db,
            czar_core::services::workflow_yaml::ApplyWorkflowYamlRequest {
                yaml_content: &yaml_content,
                root: &root,
                project: "local",
                region: "local",
                existing_workflow:
                    czar_core::services::workflow_yaml::ExistingWorkflowBehavior::Replace,
                persist_schedule_on_create: false,
                var_overrides,
                validation_options,
                registration_metadata: None,
            },
        )
        .await
        .with_context(|| "Failed to apply workflow")?;

        let workflow = result.workflow;

        println!("✓ Created workflow: {}", workflow.name);
        println!("  ID: {}", workflow.id);

        // Trigger workflow
        info!("Triggering workflow...");
        let run = if self.selection.flow.is_some() || !self.selection.selectors.is_empty() {
            let result = build_flow_selection(
                &*db,
                workflow.id,
                &yaml_content,
                Some(&root),
                self.selection.flow.as_deref(),
                &self.selection.selectors,
                validation_options,
            )
            .await?;
            db.create_run_with_snapshot_and_selection(
                workflow.id,
                Some(result.snapshot_id),
                "cli-execute",
                build_run_launch_json(result.task_selection, None)?,
            )
            .await?
        } else {
            db.create_run(workflow.id, "cli-execute").await?
        };
        println!("✓ Triggered run: {}", run.id);
        println!();

        let start_local_scheduler = Self::should_start_local_scheduler(&*db, run.id).await?;
        let scheduler_handle = if start_local_scheduler {
            // Start scheduler in background with fast config for local execution.
            info!("Starting local scheduler...");
            let executor_manager = Arc::new(ExecutorManager::new());
            let job_trackers = crate::commands::cloud::job_trackers().await?;
            let scheduler = if let Some(config_path) = self.config {
                let config_content = std::fs::read_to_string(&config_path)?;
                let mut orchestrator_config: OrchestratorConfig =
                    serde_yaml::from_str(&config_content)?;
                if orchestrator_config.notifications.is_empty() {
                    orchestrator_config.notifications = project_notifications.clone();
                }
                Scheduler::new_with_config(db.clone(), executor_manager, orchestrator_config)
                    .with_job_trackers(job_trackers)
            } else {
                let config = OrchestratorConfig {
                    poll_interval_secs: 0.01,
                    notifications: project_notifications,
                    ..OrchestratorConfig::default()
                };
                Scheduler::new_with_config(db.clone(), executor_manager, config)
                    .with_job_trackers(job_trackers)
            };

            Some(tokio::spawn(async move {
                let _ = scheduler.run().await;
            }))
        } else {
            info!("Detected external scheduler activity; skipping local scheduler startup");
            None
        };

        // Wait for completion
        let mut log_streamer = (!self.silent).then(|| RunLogStreamer::new(self.verbose));
        let result = timeout(Duration::from_secs(self.timeout), async {
            loop {
                let tasks = db.list_tasks(run.id).await?;
                if let Some(streamer) = log_streamer.as_mut() {
                    streamer.emit(&tasks);
                }
                let total = tasks.len() as i64;
                let completed = tasks
                    .iter()
                    .filter(|task| {
                        matches!(task.status(), TaskStatus::Success | TaskStatus::Skipped)
                    })
                    .count() as i64;
                let failed = tasks
                    .iter()
                    .filter(|task| {
                        matches!(task.status(), TaskStatus::Failed | TaskStatus::Cancelled)
                    })
                    .count() as i64;
                if completed + failed >= total && total > 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Ok::<(), anyhow::Error>(())
        })
        .await;

        if let Some(scheduler_handle) = scheduler_handle {
            scheduler_handle.abort();
        }

        match result {
            Ok(Ok(())) => {
                let elapsed = start_time.elapsed();
                let tasks = db.list_tasks(run.id).await?;
                if let Some(streamer) = log_streamer.as_mut() {
                    streamer.emit(&tasks);
                }

                let success_count = tasks
                    .iter()
                    .filter(|t| matches!(t.status(), TaskStatus::Success))
                    .count();
                let failed_count = tasks
                    .iter()
                    .filter(|t| matches!(t.status(), TaskStatus::Failed))
                    .count();
                let run_info = db.get_run(run.id).await?;
                let run_status = if matches!(run_info.status(), RunStatus::Cancelled) {
                    RunStatus::Cancelled
                } else if failed_count > 0 {
                    RunStatus::Failed
                } else {
                    RunStatus::Success
                };

                if !matches!(
                    run_info.status(),
                    RunStatus::Success | RunStatus::Failed | RunStatus::Cancelled
                ) {
                    let _ = db.update_run_status(run.id, run_status, None).await;
                }

                match run_status {
                    RunStatus::Success => println!(
                        "✓ Run completed successfully in {:.2}s",
                        elapsed.as_secs_f64()
                    ),
                    RunStatus::Cancelled => {
                        println!("✗ Run was cancelled after {:.2}s", elapsed.as_secs_f64())
                    }
                    _ => println!("✗ Run failed in {:.2}s", elapsed.as_secs_f64()),
                }

                println!(
                    "  Tasks: {} success, {} failed, {} total",
                    success_count,
                    failed_count,
                    tasks.len()
                );
                println!();

                // Show task details
                let format_dt = |dt: chrono::DateTime<chrono::Utc>| {
                    let base = dt.format("%Y-%m-%dT%H:%M:%S").to_string();
                    let centis = dt.timestamp_subsec_millis() / 10;
                    format!("{base}.{:02}Z", centis)
                };

                println!("Tasks:");
                println!(
                    "{:<20} {:<15} {:<10} {:<8} {:<24} {:<24}",
                    "Name", "Status", "Duration", "Attempts", "Started", "Finished"
                );
                println!("{}", "-".repeat(105));

                for task in &tasks {
                    let duration = if let (Some(started), Some(finished)) =
                        (task.started_at, task.finished_at)
                    {
                        let duration = finished.signed_duration_since(started);
                        format!("{:.2}s", duration.num_milliseconds() as f64 / 1000.0)
                    } else {
                        "-".to_string()
                    };
                    let status = task.status();
                    let attempt_count = match status {
                        TaskStatus::Success | TaskStatus::Running | TaskStatus::Dispatched => {
                            task.attempts + 1
                        }
                        TaskStatus::Failed => task.attempts,
                        _ => task.attempts,
                    };
                    let attempts_display = if attempt_count > 0 {
                        attempt_count.to_string()
                    } else {
                        "-".to_string()
                    };
                    let started = task
                        .started_at
                        .map(format_dt)
                        .unwrap_or_else(|| "-".to_string());
                    let finished = task
                        .finished_at
                        .map(format_dt)
                        .unwrap_or_else(|| "-".to_string());

                    println!(
                        "{:<20} {:<15} {:<10} {:<8} {:<24} {:<24}",
                        task.task_name,
                        status.as_str(),
                        duration,
                        attempts_display,
                        started,
                        finished
                    );
                }

                if !matches!(run_status, RunStatus::Success) {
                    println!();
                    println!("Inspect task output with: czar logs {} [task-name]", run.id);
                    return Err(anyhow!("Run completed with {} status", run_status.as_str()));
                }

                Ok(())
            }
            Ok(Err(e)) => {
                println!("✗ Error during execution: {}", e);
                Err(e)
            }
            Err(_) => {
                let msg = format!(
                    "Timeout waiting for workflow to complete after {}s",
                    self.timeout
                );
                println!("✗ {msg}");
                Err(anyhow!(msg))
            }
        }
    }
}

#[cfg(test)]
mod tests;
