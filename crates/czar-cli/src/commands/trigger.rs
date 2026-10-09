use anyhow::{Context, Result};
use clap::Args;
use czar_core::database::NewWorkflowTask;
use czar_core::models::{
    RunLaunchConfig, RunTaskSelection, TaskSelectionMode, WorkflowTask, json_inner,
};
use czar_core::task_execution::resolve_task_selection;
use czar_core::workflow::{ValidationOptions, WorkflowDefinition};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

use crate::bootstrap;
use crate::commands::selection_spec::{FlowSelectionArgs, expand_selectors};
use czar_core::database::Database;

#[derive(Args, Default)]
pub struct Trigger {
    /// Workflow name to trigger
    pub workflow_name: String,

    #[command(flatten)]
    pub selection: FlowSelectionArgs,
}

impl Trigger {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        if self.selection.flow.is_none() && self.selection.selectors.is_empty() {
            return self.execute_simple(db).await;
        }
        self.execute_with_selection(db).await
    }

    async fn execute_simple(self, db: Arc<dyn Database>) -> Result<()> {
        let workflow = db.get_workflow(&self.workflow_name).await?;
        let run = db.create_run(workflow.id, "manual").await?;

        println!("✓ Triggered workflow: {}", self.workflow_name);
        println!("  Run ID: {}", run.id);
        println!("  Status: {}", run.status_str());
        println!("\nUse 'status {}' to check progress", run.id);
        Ok(())
    }

    async fn execute_with_selection(self, db: Arc<dyn Database>) -> Result<()> {
        let workflow = db.get_workflow(&self.workflow_name).await?;
        let source_yaml =
            load_workflow_source_yaml(&*db, &workflow.name, workflow.current_snapshot_id).await?;
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
                build_run_launch_json(result.task_selection, None)?,
            )
            .await?;

        println!("✓ Triggered workflow: {}", self.workflow_name);
        println!("  Run ID: {}", run.id);
        println!("  Status: {}", run.status_str());
        println!("\nUse 'status {}' to check progress", run.id);
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct FlowSelectionResult {
    pub snapshot_id: uuid::Uuid,
    pub task_selection: Option<RunTaskSelection>,
}

/// Build a snapshot and resolve optional selectors for a flow.
/// Used by both `trigger` and `execute` to avoid duplicating this logic.
pub(crate) async fn build_flow_selection(
    db: &dyn Database,
    workflow_id: uuid::Uuid,
    yaml_content: &str,
    workflow_root: Option<&std::path::Path>,
    flow: Option<&str>,
    selectors: &[String],
    validation_options: ValidationOptions,
) -> Result<FlowSelectionResult> {
    let definition = if let Some(root) = workflow_root {
        WorkflowDefinition::parse_yaml_with_root(yaml_content, root)
            .map_err(|err| anyhow::anyhow!(err.to_string()))
    } else {
        WorkflowDefinition::parse_yaml(yaml_content).map_err(anyhow::Error::from)
    }
    .context("failed to parse workflow source YAML")?;
    definition
        .validate_catalog_with_options(validation_options)
        .map_err(|err| anyhow::anyhow!("invalid workflow definition: {err}"))?;

    let selected_flow_name = definition
        .selected_flow_name(flow)
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    let flow_workflow = definition
        .resolve_flow_with_options(flow, validation_options)
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    let flow_name_for_metadata = definition
        .flows
        .contains_key(&selected_flow_name)
        .then_some(selected_flow_name);

    let base_workflow_tasks = db.list_workflow_tasks(workflow_id).await?;
    let snapshot_tasks = build_snapshot_tasks_for_flow(&base_workflow_tasks, &flow_workflow)?;
    let snapshot_tasks_json = serde_json::to_value(&snapshot_tasks)?;
    let content_hash = compute_content_hash(&snapshot_tasks_json);
    let snapshot = db
        .create_or_get_snapshot(
            workflow_id,
            &content_hash,
            snapshot_tasks_json,
            Some(yaml_content.to_string()),
        )
        .await?;

    let task_graph: Vec<(String, Vec<String>)> = flow_workflow
        .tasks
        .iter()
        .map(|(name, task)| (name.clone(), task.depends_on.clone()))
        .collect();

    let run_selection = if selectors.is_empty() {
        flow_name_for_metadata.clone().map(|flow_name| {
            let resolved_tasks: Vec<String> =
                task_graph.iter().map(|(name, _)| name.clone()).collect();
            RunTaskSelection {
                mode: TaskSelectionMode::Exact,
                flow_name: Some(flow_name),
                requested_tasks: resolved_tasks.clone(),
                requested_groups: Vec::new(),
                resolved_tasks,
            }
        })
    } else {
        let expanded = expand_selectors(&task_graph, selectors)?;
        let resolved_tasks = resolve_task_selection(
            &task_graph,
            &expanded.resolved_tasks,
            TaskSelectionMode::Exact,
        )?;
        Some(RunTaskSelection {
            mode: TaskSelectionMode::Exact,
            flow_name: flow_name_for_metadata,
            requested_tasks: expanded.requested_tasks,
            requested_groups: Vec::new(),
            resolved_tasks,
        })
    };

    Ok(FlowSelectionResult {
        snapshot_id: snapshot.id,
        task_selection: run_selection,
    })
}

pub(crate) async fn load_workflow_source_yaml(
    db: &dyn Database,
    workflow_name: &str,
    snapshot_id: Option<uuid::Uuid>,
) -> Result<String> {
    let snapshot_id = snapshot_id.ok_or_else(|| {
        anyhow::anyhow!(
            "workflow '{}' is missing source YAML snapshot; re-apply workflow YAML before using selective trigger",
            workflow_name
        )
    })?;
    let snapshot = db.get_snapshot(snapshot_id).await?;
    snapshot.source_yaml.ok_or_else(|| {
        anyhow::anyhow!(
            "workflow '{}' has no source YAML in current snapshot; re-apply workflow YAML before using selective trigger",
            workflow_name
        )
    })
}

pub(crate) fn build_snapshot_tasks_for_flow(
    base_tasks: &[WorkflowTask],
    flow_workflow: &czar_core::workflow::Workflow,
) -> Result<Vec<NewWorkflowTask>> {
    let base_by_name: HashMap<&str, &WorkflowTask> = base_tasks
        .iter()
        .map(|task| (task.task_name.as_str(), task))
        .collect();

    let mut output = Vec::with_capacity(flow_workflow.tasks.len());
    for (index, (task_name, flow_task)) in flow_workflow.tasks.iter().enumerate() {
        let base_task = base_by_name.get(task_name.as_str()).ok_or_else(|| {
            anyhow::anyhow!(
                "flow task '{}' is missing from persisted workflow tasks",
                task_name
            )
        })?;

        let mut params = base_task
            .params
            .as_ref()
            .map(|value| json_inner(value).clone())
            .unwrap_or_else(|| serde_json::json!({}));
        if !params.is_object() {
            params = serde_json::json!({});
        }
        let params_obj = params.as_object_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "persisted params for task '{}' must be an object",
                task_name
            )
        })?;

        params_obj.insert(
            "max_retries".to_string(),
            serde_json::Value::Number(flow_task.retries.into()),
        );
        params_obj.insert(
            "timeout_seconds".to_string(),
            serde_json::Value::Number(flow_task.timeout.into()),
        );
        if flow_task.inputs.is_null() {
            params_obj.remove("task_bindings");
        } else {
            params_obj.insert("task_bindings".to_string(), flow_task.inputs.clone());
        }

        output.push(NewWorkflowTask {
            task_index: i32::try_from(index).unwrap_or(i32::MAX),
            task_name: task_name.clone(),
            executor_type: base_task.executor_type.clone(),
            depends_on: flow_task.depends_on.clone(),
            params,
            signature: None,
        });
    }

    Ok(output)
}

pub(crate) fn compute_content_hash(tasks_json: &serde_json::Value) -> String {
    let canonical = serde_json::to_string(tasks_json).unwrap_or_else(|_| "null".to_string());
    let digest = Sha256::digest(canonical.as_bytes());
    hex::encode(digest)
}

pub(crate) fn build_run_launch_json(
    task_selection: Option<RunTaskSelection>,
    inputs: Option<serde_json::Value>,
) -> Result<Option<serde_json::Value>> {
    RunLaunchConfig {
        task_selection,
        inputs,
    }
    .to_storage_value()
}

#[cfg(test)]
mod tests;
