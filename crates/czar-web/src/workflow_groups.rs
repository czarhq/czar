use std::collections::{BTreeMap, BTreeSet, HashMap};

use czar_core::database::NewWorkflowTask;
use czar_core::models::{WorkflowTask, json_inner};
use czar_core::workflow::{Workflow, WorkflowDefinition};

pub fn parse_workflow_definition_from_source_yaml(
    source_yaml: &str,
) -> Result<WorkflowDefinition, String> {
    let definition = WorkflowDefinition::parse_yaml(source_yaml)
        .map_err(|err| format!("Failed to parse source YAML: {err}"))?;
    definition
        .validate_catalog()
        .map_err(|err| format!("Invalid workflow definition: {err}"))?;
    Ok(definition)
}

pub fn parse_task_groups_from_source_yaml(
    source_yaml: &str,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    Ok(parse_workflow_definition_from_source_yaml(source_yaml)?.task_groups)
}

pub fn resolve_flow_workflow_from_source_yaml(
    source_yaml: &str,
    flow_name: Option<&str>,
) -> Result<Workflow, String> {
    let definition = parse_workflow_definition_from_source_yaml(source_yaml)?;
    definition
        .resolve_flow(flow_name)
        .map_err(|err| err.to_string())
}

pub fn expand_group_tasks(
    groups: &BTreeMap<String, Vec<String>>,
    requested_groups: &[String],
) -> Result<Vec<String>, String> {
    let mut tasks = Vec::new();
    let mut seen = BTreeSet::new();

    for group in requested_groups {
        let name = group.trim();
        if name.is_empty() {
            continue;
        }
        let Some(entries) = groups.get(name) else {
            return Err(format!("unknown task group '{name}'"));
        };
        for task in entries {
            if seen.insert(task.clone()) {
                tasks.push(task.clone());
            }
        }
    }

    Ok(tasks)
}

pub fn build_snapshot_tasks_for_flow(
    base_tasks: &[WorkflowTask],
    flow_workflow: &Workflow,
) -> Result<Vec<NewWorkflowTask>, String> {
    let base_by_name: HashMap<&str, &WorkflowTask> = base_tasks
        .iter()
        .map(|task| (task.task_name.as_str(), task))
        .collect();

    let mut output = Vec::with_capacity(flow_workflow.tasks.len());
    for (index, (task_name, flow_task)) in flow_workflow.tasks.iter().enumerate() {
        let Some(base_task) = base_by_name.get(task_name.as_str()) else {
            return Err(format!(
                "flow task '{}' is missing from persisted workflow tasks",
                task_name
            ));
        };

        let mut params = base_task
            .params
            .as_ref()
            .map(|value| json_inner(value).clone())
            .unwrap_or_else(|| serde_json::json!({}));
        if !params.is_object() {
            params = serde_json::json!({});
        }
        let Some(params_obj) = params.as_object_mut() else {
            return Err(format!(
                "persisted params for task '{}' must be an object",
                task_name
            ));
        };

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
