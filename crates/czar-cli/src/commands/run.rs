use anyhow::{Result, anyhow};
use clap::{Args, Subcommand};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use super::execute::Execute;
use crate::bootstrap;
use crate::commands::selection_spec::FlowSelectionArgs;
use crate::commands::trigger::{build_flow_selection, build_run_launch_json};
use czar_core::database::{Database, RunListQuery};
use czar_core::models::{RunStatus, TaskStatus};
use czar_core::workflow::ValidationOptions;
use uuid::Uuid;

#[derive(Args)]
pub struct Run {
    #[command(subcommand)]
    pub action: Option<RunAction>,

    /// Workflow YAML file path to execute locally
    #[arg(value_name = "WORKFLOW_FILE")]
    pub file: Option<String>,

    /// Optional config file path (YAML)
    #[arg(short, long)]
    pub config: Option<String>,

    /// Override workflow vars from the CLI (repeatable: --var NAME=VALUE)
    #[arg(long = "var", value_name = "NAME=VALUE", value_parser = parse_var_override)]
    pub vars: Vec<(String, String)>,

    #[command(flatten)]
    pub selection: FlowSelectionArgs,

    /// Maximum time to wait for completion in seconds
    #[arg(short, long, default_value = "300")]
    pub timeout: u64,

    /// Suppress live task log output while the run is executing
    #[arg(long, conflicts_with = "verbose")]
    pub silent: bool,

    /// Show task lifecycle updates in addition to live task logs
    #[arg(long, conflicts_with = "silent")]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum RunAction {
    /// Trigger an existing workflow run
    Start(RunStart),

    /// List runs, optionally filtered by workflow
    List(RunList),

    /// Show status for a specific run
    Status(RunStatusCommand),

    /// Show logs for a run
    Logs(RunLogs),

    /// Cancel a run
    Cancel(RunCancel),

    /// Reset failed portions of a run back to pending
    Retry(RunRetry),
}

impl Default for Run {
    fn default() -> Self {
        Self {
            action: None,
            file: None,
            config: None,
            vars: Vec::new(),
            selection: FlowSelectionArgs::default(),
            timeout: 300,
            silent: false,
            verbose: false,
        }
    }
}

impl Run {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        if let Some(action) = self.action {
            return action.execute(db).await;
        }

        Execute {
            file: self
                .file
                .ok_or_else(|| anyhow!("missing workflow file for local run"))?,
            config: self.config,
            vars: self.vars,
            selection: self.selection,
            timeout: self.timeout,
            silent: self.silent,
            verbose: self.verbose,
        }
        .execute(db)
        .await
    }
}

impl RunAction {
    async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        match self {
            RunAction::Start(cmd) => cmd.execute(db).await,
            RunAction::List(cmd) => cmd.execute(db).await,
            RunAction::Status(cmd) => cmd.execute(db).await,
            RunAction::Logs(cmd) => cmd.execute(db).await,
            RunAction::Cancel(cmd) => cmd.execute(db).await,
            RunAction::Retry(cmd) => cmd.execute(db).await,
        }
    }
}

#[derive(Args, Default)]
pub struct RunStart {
    /// Workflow name to trigger
    pub workflow_name: String,

    /// Override run-scoped input values (repeatable: --input NAME=VALUE)
    ///
    /// Values are parsed as JSON when possible, otherwise stored as strings.
    #[arg(
        long = "input",
        alias = "inputs",
        value_name = "NAME=VALUE",
        value_parser = parse_run_input
    )]
    pub inputs: Vec<(String, Value)>,

    #[command(flatten)]
    pub selection: FlowSelectionArgs,
}

impl RunStart {
    async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let inputs = (!self.inputs.is_empty())
            .then(|| serde_json::Value::Object(self.inputs.into_iter().collect()));

        if self.selection.flow.is_none() && self.selection.selectors.is_empty() {
            let workflow = db.get_workflow(&self.workflow_name).await?;
            let run = match build_run_launch_json(None, inputs)? {
                Some(config) => {
                    db.create_run_with_selection(workflow.id, "manual", Some(config))
                        .await?
                }
                None => db.create_run(workflow.id, "manual").await?,
            };

            println!("✓ Triggered workflow: {}", self.workflow_name);
            println!("  Run ID: {}", run.id);
            println!("  Status: {}", run.status_str());
            println!("\nUse 'status {}' to check progress", run.id);
            return Ok(());
        }

        let workflow = db.get_workflow(&self.workflow_name).await?;
        let source_yaml = super::trigger::load_workflow_source_yaml(
            &*db,
            &workflow.name,
            workflow.current_snapshot_id,
        )
        .await?;
        let validation_options =
            if bootstrap::project_contract_validation_enabled_from_current_dir()? {
                ValidationOptions::STRICT
            } else {
                ValidationOptions::RELAXED
            };

        let result = build_flow_selection(
            &*db,
            workflow.id,
            &source_yaml,
            None,
            self.selection.flow.as_deref(),
            &self.selection.selectors,
            validation_options,
        )
        .await?;

        let run = db
            .create_run_with_snapshot_and_selection(
                workflow.id,
                Some(result.snapshot_id),
                "manual",
                build_run_launch_json(result.task_selection, inputs)?,
            )
            .await?;

        println!("✓ Triggered workflow: {}", self.workflow_name);
        println!("  Run ID: {}", run.id);
        println!("  Status: {}", run.status_str());
        println!("\nUse 'status {}' to check progress", run.id);
        Ok(())
    }
}

#[derive(Args, Default)]
pub struct RunList {
    /// Workflow name to filter by
    pub workflow_name: Option<String>,

    /// Filter runs by status
    #[arg(long, value_name = "STATUS", value_parser = parse_run_status)]
    pub status: Option<RunStatus>,

    /// Limit number of runs returned
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
}

impl RunList {
    async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let query = RunListQuery {
            limit: self.limit.unwrap_or(50),
            offset: 0,
            search: None,
            statuses: self.status.into_iter().collect(),
            workflow_names: self.workflow_name.into_iter().collect(),
            since: None,
            until: None,
        };
        let page = db.list_runs_page(&query).await?;
        if page.items.is_empty() {
            println!("No runs found");
            return Ok(());
        }

        println!("Runs:");
        println!(
            "{:<36} {:<30} {:<12} {:<20} {:<20}",
            "Run ID", "Workflow", "Status", "Started", "Finished"
        );
        println!("{}", "-".repeat(122));

        for entry in page.items {
            let started = entry
                .run
                .started_at
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "-".to_string());
            let finished = entry
                .run
                .finished_at
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "-".to_string());
            println!(
                "{:<36} {:<30} {:<12} {:<20} {:<20}",
                entry.run.id,
                entry
                    .workflow_name
                    .unwrap_or_else(|| entry.run.workflow_id.to_string()),
                entry.run.status_str(),
                started,
                finished
            );
        }

        if page.total > query.limit {
            println!();
            println!("Showing {} of {} runs", query.limit, page.total);
        }

        Ok(())
    }
}

#[derive(Args, Default)]
pub struct RunStatusCommand {
    /// Run ID
    pub run_id: String,
}

impl RunStatusCommand {
    async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        super::Status {
            run_id: Some(self.run_id),
            workflow: None,
        }
        .execute(db)
        .await
    }
}

#[derive(Args, Default)]
pub struct RunLogs {
    /// Run ID
    pub run_id: String,

    /// Optional task name or task ID to inspect
    #[arg(long)]
    pub task: Option<String>,

    /// Follow logs until the run finishes
    #[arg(long)]
    pub follow: bool,
}

impl RunLogs {
    async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        super::Logs {
            run_id: self.run_id,
            task: self.task,
            follow: self.follow,
            poll_interval_ms: 1000,
        }
        .execute(db)
        .await
    }
}

#[derive(Args, Default)]
pub struct RunCancel {
    /// Run ID
    pub run_id: String,
}

impl RunCancel {
    async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let run_id: Uuid = self.run_id.parse()?;
        db.cancel_run(run_id).await?;
        println!("✓ Cancelled run: {}", run_id);
        Ok(())
    }
}

#[derive(Args, Default)]
pub struct RunRetry {
    /// Run ID
    pub run_id: String,

    /// Specific failed task name or task ID to retry from
    #[arg(long)]
    pub task: Option<String>,
}

impl RunRetry {
    async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let run_id: Uuid = self.run_id.parse()?;
        let tasks = db.list_tasks(run_id).await?;
        if tasks.is_empty() {
            return Err(anyhow!("no tasks found for run {}", run_id));
        }

        let task_map: HashMap<String, czar_core::models::Task> = tasks
            .iter()
            .cloned()
            .map(|task| (task.task_name.clone(), task))
            .collect();

        let root_names = if let Some(selector) = self.task.as_deref() {
            let task = tasks
                .iter()
                .find(|task| task.task_name == selector || task.id.to_string() == selector)
                .ok_or_else(|| {
                    anyhow!(
                        "no task named or identified by '{}' in run {}",
                        selector,
                        run_id
                    )
                })?;

            if !matches!(task.status(), TaskStatus::Failed | TaskStatus::Cancelled) {
                return Err(anyhow!(
                    "task '{}' is {}, so retry expects a failed or cancelled task root",
                    task.task_name,
                    task.status_str()
                ));
            }

            vec![task.task_name.clone()]
        } else {
            let names: Vec<String> = tasks
                .iter()
                .filter(|task| matches!(task.status(), TaskStatus::Failed | TaskStatus::Cancelled))
                .map(|task| task.task_name.clone())
                .collect();
            if names.is_empty() {
                return Err(anyhow!(
                    "run {} has no failed or cancelled tasks to retry",
                    run_id
                ));
            }
            names
        };

        let reverse_deps = build_reverse_dependencies(&tasks);
        let mut reset_names: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = root_names.iter().cloned().collect();
        while let Some(task_name) = queue.pop_front() {
            if !reset_names.insert(task_name.clone()) {
                continue;
            }
            if let Some(children) = reverse_deps.get(&task_name) {
                for child in children {
                    if let Some(task) = task_map.get(child)
                        && matches!(
                            task.status(),
                            TaskStatus::Failed | TaskStatus::Skipped | TaskStatus::Cancelled
                        )
                    {
                        queue.push_back(child.clone());
                    }
                }
            }
        }

        let mut reset_tasks = Vec::new();
        for name in &reset_names {
            let task = task_map
                .get(name)
                .ok_or_else(|| anyhow!("task '{}' disappeared during retry planning", name))?;
            db.reset_task_for_retry(task.id, Some("manual retry"), None)
                .await?;
            reset_tasks.push(task.task_name.clone());
        }

        reset_tasks.sort();
        db.update_run_status(run_id, czar_core::models::RunStatus::Pending, None)
            .await?;

        println!("✓ Reset run {} for retry", run_id);
        println!("  Tasks reset: {}", reset_tasks.join(", "));
        Ok(())
    }
}

fn build_reverse_dependencies(tasks: &[czar_core::models::Task]) -> HashMap<String, Vec<String>> {
    let mut reverse = HashMap::new();
    for task in tasks {
        for dep in &task.depends_on {
            reverse
                .entry(dep.clone())
                .or_insert_with(Vec::new)
                .push(task.task_name.clone());
        }
    }
    reverse
}

pub(crate) fn parse_var_override(raw: &str) -> Result<(String, String), String> {
    let Some((name, value)) = raw.split_once('=') else {
        return Err(format!("invalid --var '{raw}': expected NAME=VALUE"));
    };
    if name.is_empty() {
        return Err(format!("invalid --var '{raw}': missing variable name"));
    }
    Ok((name.to_string(), value.to_string()))
}

pub(crate) fn parse_run_input(raw: &str) -> Result<(String, Value), String> {
    let Some((name, value_raw)) = raw.split_once('=') else {
        return Err(format!("invalid --input '{raw}': expected NAME=VALUE"));
    };
    if name.is_empty() {
        return Err(format!("invalid --input '{raw}': missing input name"));
    }
    let value = serde_json::from_str::<Value>(value_raw)
        .unwrap_or_else(|_| Value::String(value_raw.to_string()));
    Ok((name.to_string(), value))
}

fn parse_run_status(raw: &str) -> Result<RunStatus, String> {
    raw.parse::<RunStatus>()
        .map_err(|_| format!("invalid run status: {}", raw))
}

#[cfg(test)]
mod tests;
