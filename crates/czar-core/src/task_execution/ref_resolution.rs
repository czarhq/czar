use anyhow::Result;
use std::collections::{HashMap, HashSet};

use crate::models::{TaskWithWorkflow, json_inner};

enum BindingRef<'a> {
    TaskOutput {
        dep_task: &'a str,
        field_path: Vec<&'a str>,
    },
    WorkflowVar {
        key: String,
    },
    ParallelIndex,
    ParallelCount,
    ForeachIndex,
    ForeachCount,
    ForeachItem {
        field_path: Vec<&'a str>,
    },
}

pub(super) fn resolve_ref_path(
    task: &TaskWithWorkflow,
    reference: &str,
    run_outputs: Option<&HashMap<String, serde_json::Value>>,
    run_tasks: Option<&HashSet<String>>,
    workflow_outputs: Option<&HashMap<String, serde_json::Value>>,
) -> Result<serde_json::Value> {
    match parse_binding_ref(reference)? {
        BindingRef::TaskOutput {
            dep_task,
            field_path,
        } => resolve_task_output_ref(
            task,
            reference,
            dep_task,
            &field_path,
            run_outputs,
            run_tasks,
            workflow_outputs,
        ),
        BindingRef::WorkflowVar { key } => resolve_workflow_var(task, reference, &key),
        BindingRef::ParallelIndex => resolve_context_value(task, reference, "parallel_index", &[]),
        BindingRef::ParallelCount => resolve_context_value(task, reference, "parallel_count", &[]),
        BindingRef::ForeachIndex => resolve_context_value(task, reference, "foreach_index", &[]),
        BindingRef::ForeachCount => resolve_context_value(task, reference, "foreach_count", &[]),
        BindingRef::ForeachItem { field_path } => {
            resolve_context_value(task, reference, "foreach_item", &field_path)
        }
    }
}

pub(crate) fn resolve_upstream_output_selector(
    task: &TaskWithWorkflow,
    reference: &str,
    run_outputs: Option<&HashMap<String, serde_json::Value>>,
    run_tasks: Option<&HashSet<String>>,
    workflow_outputs: Option<&HashMap<String, serde_json::Value>>,
) -> Result<serde_json::Value> {
    let (dep_task, field_path) = parse_upstream_output_selector(reference)?;
    let field_path_refs: Vec<&str> = field_path.iter().map(String::as_str).collect();
    resolve_task_output_ref(
        task,
        reference,
        &dep_task,
        &field_path_refs,
        run_outputs,
        run_tasks,
        workflow_outputs,
    )
}

pub(crate) fn parse_upstream_output_selector(reference: &str) -> Result<(String, Vec<String>)> {
    let parts: Vec<&str> = reference.split('.').collect();
    if parts.len() >= 3 && parts[0] == "tasks" && parts[2] == "output" {
        return Ok((
            parts[1].to_string(),
            parts
                .iter()
                .skip(3)
                .map(|segment| segment.to_string())
                .collect(),
        ));
    }
    if parts.len() >= 2 {
        return Ok((
            parts[0].to_string(),
            parts
                .iter()
                .skip(1)
                .map(|segment| segment.to_string())
                .collect(),
        ));
    }
    Err(anyhow::anyhow!(
        "unsupported upstream selector '{}': expected <task>.<field>[.<field>...] or tasks.<task>.output[.<field>...]",
        reference
    ))
}

fn resolve_task_output_ref(
    task: &TaskWithWorkflow,
    reference: &str,
    dep_task: &str,
    field_path: &[&str],
    run_outputs: Option<&HashMap<String, serde_json::Value>>,
    run_tasks: Option<&HashSet<String>>,
    workflow_outputs: Option<&HashMap<String, serde_json::Value>>,
) -> Result<serde_json::Value> {
    let direct_dep = task.depends_on.iter().any(|dep| dep == dep_task);

    let expanded_prefix = format!("{}[", dep_task);
    let expanded_dep_names: Vec<&String> = task
        .depends_on
        .iter()
        .filter(|dep| dep.starts_with(&expanded_prefix) && dep.ends_with(']'))
        .collect();

    if !direct_dep && expanded_dep_names.is_empty() {
        return Err(anyhow::anyhow!(
            "task '{}' ref '{}' targets task '{}' which is not listed in depends_on",
            task.task_name,
            reference,
            dep_task
        ));
    }

    if !expanded_dep_names.is_empty() {
        return resolve_parallel_group_ref(
            task,
            reference,
            dep_task,
            &expanded_dep_names,
            field_path,
            run_outputs,
            workflow_outputs,
        );
    }

    let dep_in_run = run_tasks
        .map(|names| names.contains(dep_task))
        .unwrap_or(false);
    let value = if dep_in_run {
        let outputs = run_outputs.ok_or_else(|| {
            anyhow::anyhow!(
                "task '{}' ref '{}' cannot be resolved: run outputs are unavailable",
                task.task_name,
                reference
            )
        })?;
        outputs.get(dep_task).cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "task '{}' ref '{}' cannot be resolved: dependency '{}' is in this run but has no output yet",
                task.task_name,
                reference,
                dep_task
            )
        })?
    } else if let Some(outputs) = run_outputs {
        if let Some(v) = outputs.get(dep_task) {
            v.clone()
        } else if let Some(v) = workflow_outputs.and_then(|outputs| outputs.get(dep_task)) {
            v.clone()
        } else {
            return Err(anyhow::anyhow!(
                "task '{}' ref '{}' cannot be resolved: dependency '{}' is not in this run and has no persisted successful output",
                task.task_name,
                reference,
                dep_task
            ));
        }
    } else if let Some(v) = workflow_outputs.and_then(|outputs| outputs.get(dep_task)) {
        v.clone()
    } else {
        return Err(anyhow::anyhow!(
            "task '{}' ref '{}' cannot be resolved: no run outputs and no persisted successful output for dependency '{}'",
            task.task_name,
            reference,
            dep_task
        ));
    };

    extract_field_path(value, field_path, || {
        format!("task '{}' ref '{}'", task.task_name, reference)
    })
}

fn resolve_parallel_group_ref(
    task: &TaskWithWorkflow,
    reference: &str,
    dep_task: &str,
    instance_names: &[&String],
    field_path: &[&str],
    run_outputs: Option<&HashMap<String, serde_json::Value>>,
    workflow_outputs: Option<&HashMap<String, serde_json::Value>>,
) -> Result<serde_json::Value> {
    let mut sorted_names: Vec<&String> = instance_names.to_vec();
    sorted_names.sort_by_key(|name| {
        name.trim_start_matches(&format!("{}[", dep_task))
            .trim_end_matches(']')
            .parse::<usize>()
            .unwrap_or(0)
    });

    let mut outputs = Vec::with_capacity(sorted_names.len());
    for instance_name in &sorted_names {
        let instance_output = run_outputs
            .and_then(|map| map.get(*instance_name))
            .or_else(|| workflow_outputs.and_then(|map| map.get(*instance_name)))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "task '{}' ref '{}' cannot be resolved: parallel instance '{}' has no output",
                    task.task_name,
                    reference,
                    instance_name
                )
            })?;
        let value = extract_field_path(instance_output.clone(), field_path, || {
            format!(
                "task '{}' ref '{}' in instance '{}'",
                task.task_name, reference, instance_name
            )
        })?;
        outputs.push(value);
    }

    Ok(serde_json::Value::Array(outputs))
}

fn parse_binding_ref(reference: &str) -> Result<BindingRef<'_>> {
    let parts: Vec<&str> = reference.split('.').collect();
    if parts.len() >= 3 && parts[0] == "tasks" && parts[2] == "output" {
        return Ok(BindingRef::TaskOutput {
            dep_task: parts[1],
            field_path: parts.into_iter().skip(3).collect(),
        });
    }
    if parts.len() >= 2 && parts[0] == "vars" {
        return Ok(BindingRef::WorkflowVar {
            key: parts.into_iter().skip(1).collect::<Vec<_>>().join("."),
        });
    }
    if parts.as_slice() == ["parallel", "index"] {
        return Ok(BindingRef::ParallelIndex);
    }
    if parts.as_slice() == ["parallel", "count"] {
        return Ok(BindingRef::ParallelCount);
    }
    if parts.as_slice() == ["foreach", "index"] {
        return Ok(BindingRef::ForeachIndex);
    }
    if parts.as_slice() == ["foreach", "count"] {
        return Ok(BindingRef::ForeachCount);
    }
    if parts.len() >= 2 && parts[0] == "foreach" && parts[1] == "item" {
        return Ok(BindingRef::ForeachItem {
            field_path: parts.into_iter().skip(2).collect(),
        });
    }
    Err(anyhow::anyhow!(
        "unsupported ref '{}': expected tasks.<task>.output[.<field>...], vars.<NAME>, parallel.index, parallel.count, foreach.item[.<field>...], foreach.index, or foreach.count",
        reference
    ))
}

fn resolve_workflow_var(
    task: &TaskWithWorkflow,
    reference: &str,
    key: &str,
) -> Result<serde_json::Value> {
    let params = task.params.as_ref().map(json_inner).ok_or_else(|| {
        anyhow::anyhow!(
            "task '{}' ref '{}' cannot be resolved: task params are unavailable",
            task.task_name,
            reference
        )
    })?;
    let vars = params
        .get("workflow_vars")
        .and_then(|value| value.as_object())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "task '{}' ref '{}' cannot be resolved: workflow vars are unavailable",
                task.task_name,
                reference
            )
        })?;
    let value = vars.get(key).cloned().ok_or_else(|| {
        anyhow::anyhow!(
            "task '{}' ref '{}' cannot be resolved: workflow var '{}' is unavailable",
            task.task_name,
            reference,
            key
        )
    })?;
    Ok(value)
}

fn resolve_context_value(
    task: &TaskWithWorkflow,
    reference: &str,
    key: &str,
    field_path: &[&str],
) -> Result<serde_json::Value> {
    let params = task.params.as_ref().map(json_inner).ok_or_else(|| {
        anyhow::anyhow!(
            "task '{}' ref '{}' cannot be resolved: task params are unavailable",
            task.task_name,
            reference
        )
    })?;
    let value = params.get(key).cloned().ok_or_else(|| {
        anyhow::anyhow!(
            "task '{}' ref '{}' cannot be resolved: '{}' is unavailable for this task instance",
            task.task_name,
            reference,
            key
        )
    })?;
    extract_field_path(value, field_path, || {
        format!("task '{}' ref '{}'", task.task_name, reference)
    })
}

fn extract_field_path<F>(
    mut value: serde_json::Value,
    field_path: &[&str],
    context: F,
) -> Result<serde_json::Value>
where
    F: Fn() -> String,
{
    for segment in field_path {
        let obj = value.as_object().ok_or_else(|| {
            anyhow::anyhow!(
                "{} cannot access field '{}' on non-object value",
                context(),
                segment
            )
        })?;
        value = obj
            .get(*segment)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("{} missing field '{}'", context(), segment))?;
    }
    Ok(value)
}
