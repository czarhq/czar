use super::super::workflow_validation_helpers::resolve_field_schema;
use super::super::*;

impl Workflow {
    pub(super) fn validate_parallelism(&self) -> Result<(), WorkflowValidationError> {
        for (name, task) in &self.tasks {
            if task.parallel.is_some() && task.foreach.is_some() {
                return Err(WorkflowValidationError::Custom(format!(
                    "task '{}' cannot define both 'parallel' and 'foreach'",
                    name
                )));
            }

            if task.parallel_mode == ParallelMode::CloudRunTasks
                && !matches!(task.executor, ExecutorKind::CloudRun)
            {
                return Err(WorkflowValidationError::Custom(format!(
                    "task '{}' uses 'parallel_mode: cloud_run_tasks' but executor '{}' does not support it; use executor 'cloudrun'",
                    name,
                    match &task.executor {
                        ExecutorKind::Container => "container",
                        ExecutorKind::Process => "process",
                        ExecutorKind::Python => "python",
                        ExecutorKind::CloudRun => "cloudrun",
                        ExecutorKind::Library => "library",
                        ExecutorKind::Wasm => "wasm",
                        ExecutorKind::Custom(alias) => alias.as_str(),
                    }
                )));
            }

            if task.foreach.is_some() && task.parallel_mode == ParallelMode::CloudRunTasks {
                return Err(WorkflowValidationError::Custom(format!(
                    "task '{}' uses 'foreach' with 'parallel_mode: cloud_run_tasks', but Cloud Run native task counts cannot supply per-item inputs",
                    name
                )));
            }

            if task.concurrency.is_some() && task.parallel.is_none() && task.foreach.is_none() {
                return Err(WorkflowValidationError::Custom(format!(
                    "task '{}' defines 'concurrency' without 'parallel' or 'foreach'",
                    name
                )));
            }

            match task.parallel.as_ref() {
                Some(ParallelSpec::Count(0)) => {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' has invalid 'parallel: 0'; parallel count must be at least 1",
                        name
                    )));
                }
                Some(ParallelSpec::Ref(reference)) => {
                    self.validate_runtime_fanout_selector(name, task, reference, "parallel", true)?;
                }
                Some(ParallelSpec::Count(_)) | None => {}
            }

            if let Some(reference) = task.foreach.as_deref() {
                self.validate_runtime_fanout_selector(name, task, reference, "foreach", false)?;
            }
        }

        Ok(())
    }

    fn validate_runtime_fanout_selector(
        &self,
        task_name: &str,
        task: &TaskDefinition,
        reference: &str,
        field_name: &str,
        expect_int: bool,
    ) -> Result<(), WorkflowValidationError> {
        let (dep_task, fields) = parse_runtime_fanout_selector(reference).map_err(|err| {
            WorkflowValidationError::Custom(format!(
                "task '{}' has invalid '{}: {}': {}",
                task_name, field_name, reference, err
            ))
        })?;

        if !task.depends_on.iter().any(|dep| dep == &dep_task) {
            return Err(WorkflowValidationError::Custom(format!(
                "task '{}' {} reference '{}' targets '{}' which is not listed in depends_on",
                task_name, field_name, reference, dep_task
            )));
        }

        if let Some(dep_task_def) = self.tasks.get(&dep_task)
            && let Some(output_type) = dep_task_def.output_type.as_ref()
        {
            let leaf_schema = resolve_field_schema(output_type, &fields, &dep_task, &self.types)
                .map_err(|err| {
                    WorkflowValidationError::Custom(format!(
                        "task '{}' {} reference '{}' is invalid: {}",
                        task_name, field_name, reference, err
                    ))
                })?;
            if expect_int {
                match leaf_schema {
                    serde_json::Value::String(s) if s == "int" => {}
                    _ => {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' parallel reference '{}' must resolve to an int output field",
                            task_name, reference
                        )));
                    }
                }
            } else if !leaf_schema.is_array() {
                return Err(WorkflowValidationError::Custom(format!(
                    "task '{}' foreach reference '{}' must resolve to an array output field",
                    task_name, reference
                )));
            }
        }

        Ok(())
    }
}

pub(super) fn parse_runtime_fanout_selector(
    reference: &str,
) -> Result<(String, Vec<String>), String> {
    let parts: Vec<&str> = reference.split('.').collect();
    if parts.len() >= 3 && parts[0] == "tasks" && parts[2] == "output" {
        let fields = parts
            .iter()
            .skip(3)
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>();
        if fields.is_empty() {
            return Err("selector must reference a concrete output field".to_string());
        }
        return Ok((parts[1].to_string(), fields));
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
    Err("expected <task>.<field>[.<field>...] or tasks.<task>.output[.<field>...]".to_string())
}
