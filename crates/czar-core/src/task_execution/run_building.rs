use anyhow::Result;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::database::NewTask;
use crate::models::{RunTaskSelection, TaskSelectionMode, Workflow, WorkflowTask, json_inner};

/// Executor type used to mark hook entries stored alongside workflow tasks.
pub const HOOK_EXECUTOR_TYPE: &str = "hook";

pub fn build_run_tasks(
    run_id: Uuid,
    workflow: &Workflow,
    workflow_tasks: &[WorkflowTask],
    run_inputs: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Vec<NewTask> {
    workflow_tasks
        .iter()
        // Skip hook entries — they are dispatched on-demand by the scheduler.
        .filter(|task| task.executor_type != HOOK_EXECUTOR_TYPE)
        .map(|task| {
            let mut params = task
                .params
                .as_ref()
                .map(|p| json_inner(p).clone())
                .unwrap_or_else(|| serde_json::json!({}));
            if !params.is_object() {
                params = serde_json::json!({});
            }
            let obj = params.as_object_mut().expect("params object");
            merge_run_inputs_into_workflow_vars(obj, run_inputs);
            let max_retries = obj
                .get("max_retries")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .clamp(0, i64::from(i32::MAX)) as i32;
            let timeout_seconds = obj
                .get("timeout_seconds")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(0, i64::from(i32::MAX)) as i32);
            obj.entry("task_index".to_string())
                .or_insert_with(|| serde_json::json!(task.task_index));
            obj.entry("task_name".to_string())
                .or_insert_with(|| serde_json::json!(task.task_name.clone()));
            obj.entry("workflow_name".to_string())
                .or_insert_with(|| serde_json::json!(workflow.name.clone()));
            obj.entry("run_id".to_string())
                .or_insert_with(|| serde_json::json!(run_id.to_string()));

            NewTask {
                task_index: task.task_index,
                task_name: task.task_name.clone(),
                executor_type: task.executor_type.clone(),
                depends_on: task.depends_on.clone(),
                params,
                max_retries,
                timeout_seconds,
            }
        })
        .collect()
}

pub fn build_run_tasks_from_snapshot(
    run_id: Uuid,
    workflow: &Workflow,
    snapshot_tasks: &[crate::database::NewWorkflowTask],
    run_inputs: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Vec<NewTask> {
    snapshot_tasks
        .iter()
        // Skip hook entries — they are dispatched on-demand by the scheduler.
        .filter(|task| task.executor_type != HOOK_EXECUTOR_TYPE)
        .map(|task| {
            let mut params = task.params.clone();
            if !params.is_object() {
                params = serde_json::json!({});
            }
            let obj = params.as_object_mut().expect("params object");
            merge_run_inputs_into_workflow_vars(obj, run_inputs);
            let max_retries = obj
                .get("max_retries")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .clamp(0, i64::from(i32::MAX)) as i32;
            let timeout_seconds = obj
                .get("timeout_seconds")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(0, i64::from(i32::MAX)) as i32);
            obj.entry("task_index".to_string())
                .or_insert_with(|| serde_json::json!(task.task_index));
            obj.entry("task_name".to_string())
                .or_insert_with(|| serde_json::json!(task.task_name.clone()));
            obj.entry("workflow_name".to_string())
                .or_insert_with(|| serde_json::json!(workflow.name.clone()));
            obj.entry("run_id".to_string())
                .or_insert_with(|| serde_json::json!(run_id.to_string()));

            NewTask {
                task_index: task.task_index,
                task_name: task.task_name.clone(),
                executor_type: task.executor_type.clone(),
                depends_on: task.depends_on.clone(),
                params,
                max_retries,
                timeout_seconds,
            }
        })
        .collect()
}

fn merge_run_inputs_into_workflow_vars(
    params_obj: &mut serde_json::Map<String, serde_json::Value>,
    run_inputs: Option<&serde_json::Map<String, serde_json::Value>>,
) {
    let Some(run_inputs) = run_inputs else {
        return;
    };
    if run_inputs.is_empty() {
        return;
    }

    let workflow_vars = params_obj
        .entry("workflow_vars".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !workflow_vars.is_object() {
        *workflow_vars = serde_json::json!({});
    }
    let vars_obj = workflow_vars
        .as_object_mut()
        .expect("workflow_vars object after normalization");
    for (key, value) in run_inputs {
        vars_obj.insert(key.clone(), value.clone());
    }
}

pub fn resolve_task_selection(
    task_graph: &[(String, Vec<String>)],
    requested_tasks: &[String],
    mode: TaskSelectionMode,
) -> Result<Vec<String>> {
    if requested_tasks.is_empty() {
        return Err(anyhow::anyhow!("task selection requires at least one task"));
    }

    let order: Vec<String> = task_graph.iter().map(|(name, _)| name.clone()).collect();
    let known: HashSet<String> = order.iter().cloned().collect();

    let mut requested: HashSet<String> = HashSet::new();
    for name in requested_tasks {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if !known.contains(name) {
            return Err(anyhow::anyhow!("unknown task in selection: '{}'", name));
        }
        requested.insert(name.to_string());
    }
    if requested.is_empty() {
        return Err(anyhow::anyhow!("task selection is empty after trimming"));
    }

    let deps_map: HashMap<String, Vec<String>> = task_graph.iter().cloned().collect();
    let mut reverse: HashMap<String, Vec<String>> = HashMap::new();
    for (task, deps) in task_graph {
        for dep in deps {
            reverse.entry(dep.clone()).or_default().push(task.clone());
        }
    }

    let upstream = closure(&requested, |name| {
        deps_map.get(name).cloned().unwrap_or_default()
    });
    let downstream = closure(&requested, |name| {
        reverse.get(name).cloned().unwrap_or_default()
    });
    let full = closure(&downstream, |name| {
        deps_map.get(name).cloned().unwrap_or_default()
    });

    let resolved = match mode {
        TaskSelectionMode::Exact => requested,
        TaskSelectionMode::Upstream => upstream,
        TaskSelectionMode::Downstream => downstream,
        TaskSelectionMode::Full => full,
    };

    let ordered = order
        .into_iter()
        .filter(|name| resolved.contains(name))
        .collect();
    Ok(ordered)
}

pub fn filter_run_tasks_by_selection(
    tasks: Vec<NewTask>,
    selection: &RunTaskSelection,
) -> Result<Vec<NewTask>> {
    let include: HashSet<String> = selection.resolved_tasks.iter().cloned().collect();
    if include.is_empty() {
        return Err(anyhow::anyhow!("run task selection has no resolved tasks"));
    }

    let filtered: Vec<NewTask> = tasks
        .into_iter()
        .filter(|task| include.contains(&task.task_name))
        .collect();

    if filtered.len() != include.len() {
        let existing: HashSet<String> =
            filtered.iter().map(|task| task.task_name.clone()).collect();
        let mut missing: Vec<String> = include.difference(&existing).cloned().collect();
        missing.sort();
        return Err(anyhow::anyhow!(
            "run selection references tasks that do not exist in this run snapshot: {}",
            missing.join(", ")
        ));
    }

    Ok(filtered)
}

fn closure<F>(seed: &HashSet<String>, neighbors: F) -> HashSet<String>
where
    F: Fn(&str) -> Vec<String>,
{
    let mut out = seed.clone();
    let mut stack: Vec<String> = seed.iter().cloned().collect();
    while let Some(current) = stack.pop() {
        for next in neighbors(&current) {
            if out.insert(next.clone()) {
                stack.push(next);
            }
        }
    }
    out
}
