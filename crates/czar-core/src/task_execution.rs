use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

use crate::condition;
use crate::models::TaskStatus;
use crate::models::{TaskWithWorkflow, Workflow, json_inner};

use crate::executor::StatusUpdate;
use crate::executor_manager::ExecutorManager;

/// Outcome of attempting to dispatch a task.
#[derive(Debug, PartialEq)]
pub enum TaskOutcome {
    /// Task was dispatched; the string is the execution name for tracking.
    Dispatched(String),
    /// Task was skipped because its `when` condition evaluated to `false`,
    /// or because an upstream dependency was skipped.
    Skipped,
}

mod ref_resolution;
pub(crate) use ref_resolution::resolve_upstream_output_selector;

pub async fn execute_task<E: ExecutorManager>(
    task_with_workflow: TaskWithWorkflow,
    workflow_map: Arc<HashMap<Uuid, Workflow>>,
    outputs_by_run: Arc<HashMap<Uuid, HashMap<String, serde_json::Value>>>,
    available_run_tasks_by_run: Arc<HashMap<Uuid, HashSet<String>>>,
    dependency_statuses_by_run: Arc<HashMap<Uuid, HashMap<String, TaskStatus>>>,
    persisted_outputs_by_workflow: Arc<HashMap<Uuid, HashMap<String, serde_json::Value>>>,
    executor_manager: &E,
    status_tx: tokio::sync::mpsc::UnboundedSender<StatusUpdate>,
) -> (Uuid, Result<TaskOutcome>) {
    if !matches!(task_with_workflow.task_status, TaskStatus::Pending) {
        let err = anyhow::anyhow!(
            "refusing to dispatch task {} in non-pending status {}",
            task_with_workflow.task_id,
            task_with_workflow.task_status.as_str()
        );
        return (task_with_workflow.task_id, Err(err));
    }

    let workflow = match workflow_map.get(&task_with_workflow.workflow_id) {
        Some(workflow) => workflow,
        None => {
            let err = anyhow::anyhow!(
                "workflow {} not found for task {}",
                task_with_workflow.workflow_id,
                task_with_workflow.task_id
            );
            return (task_with_workflow.task_id, Err(err));
        }
    };

    let job_name = resolve_job_name(&task_with_workflow);
    let mut params = task_with_workflow
        .params
        .as_ref()
        .map(|p| json_inner(p).clone())
        .unwrap_or_else(|| serde_json::json!({}));
    if !params.is_object() {
        params = serde_json::json!({});
    }
    if let Some(obj) = params.as_object_mut() {
        let attempt_number = task_with_workflow.attempts.saturating_add(1).max(1);
        let env_value = obj
            .entry("env".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if !env_value.is_object() {
            *env_value = serde_json::json!({});
        }
        if let Some(env_obj) = env_value.as_object_mut() {
            env_obj.insert(
                "CZAR_ATTEMPT".to_string(),
                serde_json::json!(attempt_number),
            );
        }
    }

    if let Some(statuses) = dependency_statuses_by_run.get(&task_with_workflow.run_id) {
        let allow_failed_dependencies = params
            .as_object()
            .map(task_runs_despite_failed_dependencies)
            .unwrap_or(false);
        for dep in &task_with_workflow.depends_on {
            let dep_status = statuses.get(dep);
            // A skipped upstream propagates as skipped to this task
            if matches!(dep_status, Some(TaskStatus::Skipped)) {
                return (task_with_workflow.task_id, Ok(TaskOutcome::Skipped));
            }
            if matches!(dep_status, Some(TaskStatus::Failed | TaskStatus::Cancelled))
                && !allow_failed_dependencies
            {
                let err = anyhow::anyhow!(
                    "task '{}' blocked: upstream dependency '{}' failed",
                    task_with_workflow.task_name,
                    dep
                );
                return (task_with_workflow.task_id, Err(err));
            }
        }
    }

    // Evaluate `when` condition if present. The condition references upstream
    // task outputs which are available after the dependency status checks above.
    let condition_expr = params
        .as_object()
        .and_then(|obj| obj.get("condition_expr"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    if let Some(expr_str) = condition_expr {
        let expr = match condition::parse_condition(&expr_str) {
            Ok(e) => e,
            Err(e) => {
                return (
                    task_with_workflow.task_id,
                    Err(anyhow::anyhow!(
                        "task '{}' has unparseable condition: {}",
                        task_with_workflow.task_name,
                        e
                    )),
                );
            }
        };
        let run_outputs = outputs_by_run
            .get(&task_with_workflow.run_id)
            .cloned()
            .unwrap_or_default();
        match condition::evaluate_condition(&expr, &run_outputs) {
            Ok(true) => {} // condition met, proceed with execution
            Ok(false) => {
                return (task_with_workflow.task_id, Ok(TaskOutcome::Skipped));
            }
            Err(e) => {
                return (
                    task_with_workflow.task_id,
                    Err(anyhow::anyhow!(
                        "task '{}' condition evaluation failed: {}",
                        task_with_workflow.task_name,
                        e
                    )),
                );
            }
        }
    }

    if let Some(obj) = params.as_object_mut()
        && let Err(err) = resolve_bindings_for_task(
            obj,
            &task_with_workflow,
            outputs_by_run.as_ref(),
            available_run_tasks_by_run.as_ref(),
            dependency_statuses_by_run
                .get(&task_with_workflow.run_id)
                .map(|statuses| statuses as &HashMap<String, TaskStatus>),
            persisted_outputs_by_workflow.as_ref(),
        )
    {
        return (task_with_workflow.task_id, Err(err));
    }
    if let Some(obj) = params.as_object_mut()
        && let Err(err) = resolve_env_templates_for_task(
            obj,
            &task_with_workflow,
            outputs_by_run.as_ref(),
            available_run_tasks_by_run.as_ref(),
            persisted_outputs_by_workflow.as_ref(),
        )
    {
        return (task_with_workflow.task_id, Err(err));
    }

    let execution_result = match executor_manager
        .get_executor(&task_with_workflow.executor_type, workflow)
        .await
    {
        Ok(executor) => {
            executor.set_status_channel(status_tx).await;
            executor
                .execute(task_with_workflow.task_id, &job_name, Some(params))
                .await
        }
        Err(e) => Err(e),
    };

    (
        task_with_workflow.task_id,
        execution_result.map(TaskOutcome::Dispatched),
    )
}

pub fn resolve_bindings_for_task(
    params_obj: &mut serde_json::Map<String, serde_json::Value>,
    task: &TaskWithWorkflow,
    outputs_by_run: &HashMap<Uuid, HashMap<String, serde_json::Value>>,
    available_run_tasks_by_run: &HashMap<Uuid, HashSet<String>>,
    dependency_statuses: Option<&HashMap<String, TaskStatus>>,
    persisted_outputs_by_workflow: &HashMap<Uuid, HashMap<String, serde_json::Value>>,
) -> Result<()> {
    let Some(bindings_value) = params_obj.get("task_bindings").cloned() else {
        return Ok(());
    };
    let bindings_obj = bindings_value.as_object().ok_or_else(|| {
        anyhow::anyhow!(
            "task_bindings must be an object for task '{}'",
            task.task_name
        )
    })?;

    let run_outputs = outputs_by_run.get(&task.run_id);
    let run_tasks = available_run_tasks_by_run.get(&task.run_id);
    let allow_failed_dependencies = task_runs_despite_failed_dependencies(params_obj);
    let workflow_outputs = persisted_outputs_by_workflow.get(&task.workflow_id);
    let mut resolved_input = serde_json::Map::new();

    for (arg, binding) in bindings_obj {
        let binding_obj = binding.as_object().ok_or_else(|| {
            anyhow::anyhow!(
                "task '{}' binding '{}' must be an object with one of: const, ref",
                task.task_name,
                arg
            )
        })?;
        let has_const = binding_obj.contains_key("const");
        let has_ref = binding_obj.contains_key("ref");
        match (has_const, has_ref) {
            (true, false) => {
                resolved_input.insert(
                    arg.clone(),
                    binding_obj
                        .get("const")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                );
            }
            (false, true) => {
                let ref_path =
                    binding_obj
                        .get("ref")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "task '{}' binding '{}.ref' must be a string path",
                                task.task_name,
                                arg
                            )
                        })?;
                let value = ref_resolution::resolve_ref_path(
                    task,
                    ref_path,
                    run_outputs,
                    run_tasks,
                    workflow_outputs,
                )
                .or_else(|err| {
                    if allow_failed_dependencies
                        && failed_dependency_for_nullable_ref(ref_path, dependency_statuses)
                    {
                        return Ok(serde_json::Value::Null);
                    }
                    Err(err)
                })?;
                resolved_input.insert(arg.clone(), value);
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "task '{}' binding '{}' must define exactly one of: const, ref",
                    task.task_name,
                    arg
                ));
            }
        }
    }

    params_obj.insert(
        "task_input".to_string(),
        serde_json::Value::Object(resolved_input),
    );
    Ok(())
}

fn task_runs_despite_failed_dependencies(
    params_obj: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    matches!(
        params_obj
            .get("on_failure")
            .and_then(|value| value.as_str()),
        Some("run")
    )
}

fn failed_dependency_for_nullable_ref(
    ref_path: &str,
    dependency_statuses: Option<&HashMap<String, TaskStatus>>,
) -> bool {
    if !ref_path.starts_with("tasks.") {
        return false;
    }
    let Some((dep_task, _field_path)) =
        ref_resolution::parse_upstream_output_selector(ref_path).ok()
    else {
        return false;
    };
    matches!(
        dependency_statuses.and_then(|statuses| statuses.get(dep_task.as_str())),
        Some(TaskStatus::Failed | TaskStatus::Cancelled)
    )
}

fn resolve_env_templates_for_task(
    params_obj: &mut serde_json::Map<String, serde_json::Value>,
    task: &TaskWithWorkflow,
    outputs_by_run: &HashMap<Uuid, HashMap<String, serde_json::Value>>,
    available_run_tasks_by_run: &HashMap<Uuid, HashSet<String>>,
    persisted_outputs_by_workflow: &HashMap<Uuid, HashMap<String, serde_json::Value>>,
) -> Result<()> {
    let Some(env_value) = params_obj.get_mut("env") else {
        return Ok(());
    };
    let Some(env_obj) = env_value.as_object_mut() else {
        return Ok(());
    };

    let run_outputs = outputs_by_run.get(&task.run_id);
    let run_tasks = available_run_tasks_by_run.get(&task.run_id);
    let workflow_outputs = persisted_outputs_by_workflow.get(&task.workflow_id);

    for (env_key, env_entry) in env_obj.iter_mut() {
        let Some(raw) = env_entry.as_str() else {
            continue;
        };
        let Some(reference) = normalize_env_template_reference(raw, task) else {
            continue;
        };
        let resolved = ref_resolution::resolve_ref_path(
            task,
            &reference,
            run_outputs,
            run_tasks,
            workflow_outputs,
        )
        .map_err(|err| {
            anyhow::anyhow!(
                "task '{}' env '{}' failed to resolve '{}': {}",
                task.task_name,
                env_key,
                raw,
                err
            )
        })?;
        if !(resolved.is_string() || resolved.is_number() || resolved.is_boolean()) {
            return Err(anyhow::anyhow!(
                "task '{}' env '{}' resolved from '{}' must be a string, number, or boolean",
                task.task_name,
                env_key,
                raw
            ));
        }
        *env_entry = resolved;
    }

    Ok(())
}

fn normalize_env_template_reference(raw: &str, task: &TaskWithWorkflow) -> Option<String> {
    let trimmed = raw.trim();
    let inner = trimmed.strip_prefix("{{")?.strip_suffix("}}")?.trim();
    if inner.is_empty() {
        return None;
    }
    if inner.starts_with("tasks.")
        || inner.starts_with("vars.")
        || inner.starts_with("parallel.")
        || inner.starts_with("foreach.")
    {
        return Some(inner.to_string());
    }

    let (task_name, rest) = inner.split_once('.')?;
    if rest.is_empty() {
        return None;
    }
    if task.depends_on.iter().any(|dep| dep == task_name) {
        return Some(format!("tasks.{task_name}.output.{rest}"));
    }
    Some(inner.to_string())
}

fn resolve_job_name(task: &TaskWithWorkflow) -> String {
    let params = task.params.as_ref().map(json_inner);
    let override_name = params
        .and_then(|p| p.get("job_name").and_then(|v| v.as_str()))
        .or_else(|| params.and_then(|p| p.get("command").and_then(|v| v.as_str())))
        .or_else(|| params.and_then(|p| p.get("script").and_then(|v| v.as_str())));

    override_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| task.job_name.clone())
}

pub fn retry_backoff_seconds(attempt: i32) -> u64 {
    if attempt <= 1 {
        return 1;
    }
    let capped = attempt.clamp(1, 10) as u32;
    let backoff = 2_u64.saturating_pow(capped - 1);
    backoff.min(60)
}

mod run_building;
pub use run_building::{
    HOOK_EXECUTOR_TYPE, build_run_tasks, build_run_tasks_from_snapshot,
    filter_run_tasks_by_selection, resolve_task_selection,
};

#[cfg(test)]
mod tests;
