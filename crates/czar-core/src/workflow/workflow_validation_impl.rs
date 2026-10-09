use super::workflow_validation_helpers::{detect_cycle, validate_field_path, validate_type_schema};
use super::*;
use crate::condition;

mod hooks;
mod parallelism;

fn validate_env_value(
    task_name: &str,
    env_key: &str,
    env_value: &serde_json::Value,
) -> Result<(), WorkflowValidationError> {
    if env_value.is_string() || env_value.is_number() || env_value.is_boolean() {
        return Ok(());
    }

    if env_value.is_object() {
        return Err(WorkflowValidationError::Custom(format!(
            "task '{}' env '{}' must be a string, number, or boolean literal",
            task_name, env_key
        )));
    }

    Err(WorkflowValidationError::Custom(format!(
        "task '{}' env '{}' must be a string, number, or boolean literal",
        task_name, env_key
    )))
}

fn validate_secret_mount(
    task_name: &str,
    mount: &SecretFileMount,
) -> Result<(), WorkflowValidationError> {
    if mount.name.trim().is_empty() {
        return Err(WorkflowValidationError::Custom(format!(
            "task '{}' secret mounts require a non-empty 'name'",
            task_name
        )));
    }
    if mount.secret.trim().is_empty() {
        return Err(WorkflowValidationError::Custom(format!(
            "task '{}' secret mount '{}' requires a non-empty 'secret'",
            task_name, mount.name
        )));
    }
    if mount.path.as_os_str().is_empty() {
        return Err(WorkflowValidationError::Custom(format!(
            "task '{}' secret mount '{}' requires a non-empty 'path'",
            task_name, mount.name
        )));
    }
    Ok(())
}

impl Workflow {
    pub fn load(path: &Path) -> CzarResult<Self> {
        let data = std::fs::read_to_string(path).map_err(|source| CzarError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;

        let workflow: Workflow =
            serde_yaml::from_str(&data).map_err(|source| CzarError::YamlParse {
                path: path.to_path_buf(),
                source,
            })?;

        workflow
            .validate_with_options(ValidationOptions::STRICT)
            .map_err(CzarError::InvalidWorkflow)?;
        Ok(workflow)
    }

    pub fn validate(&self) -> Result<(), WorkflowValidationError> {
        self.validate_with_options(ValidationOptions::STRICT)
    }

    pub fn validate_with_options(
        &self,
        options: ValidationOptions,
    ) -> Result<(), WorkflowValidationError> {
        if self.name.trim().is_empty() {
            return Err(WorkflowValidationError::EmptyWorkflowName);
        }
        if let Some(description) = &self.description
            && description.trim().is_empty()
        {
            return Err(WorkflowValidationError::Custom(
                "workflow description cannot be empty".to_string(),
            ));
        }
        if self.tasks.is_empty() {
            return Err(WorkflowValidationError::NoTasks);
        }

        for (name, task) in &self.tasks {
            if name.trim().is_empty() {
                return Err(WorkflowValidationError::EmptyTaskName);
            }

            for dep in &task.depends_on {
                if !self.tasks.contains_key(dep) {
                    return Err(WorkflowValidationError::UnknownDependency {
                        task: name.clone(),
                        dependency: dep.clone(),
                    });
                }
                if dep == name {
                    return Err(WorkflowValidationError::SelfDependency { task: name.clone() });
                }
            }

            match task.executor {
                ExecutorKind::Container => {
                    if task.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            name
                        )));
                    }
                    if task.image.as_deref().unwrap_or_default().trim().is_empty() {
                        return Err(WorkflowValidationError::MissingTaskImage {
                            task: name.clone(),
                        });
                    }
                    if task.file.is_some()
                        || task.module.is_some()
                        || task.function.is_some()
                        || task.job.is_some()
                    {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' with executor 'container' cannot set file, module, function, or job",
                            name
                        )));
                    }
                }
                ExecutorKind::Python => {
                    if task.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            name
                        )));
                    }
                    let has_module = task
                        .module
                        .as_deref()
                        .map(|module| !module.is_empty())
                        .unwrap_or(false);
                    if task.file.is_none() && !has_module {
                        return Err(WorkflowValidationError::MissingTaskFile {
                            task: name.clone(),
                        });
                    }
                }
                ExecutorKind::Process => {
                    if task.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            name
                        )));
                    }
                    if task.command.is_none() && task.file.is_none() {
                        return Err(WorkflowValidationError::MissingTaskCommand {
                            task: name.clone(),
                        });
                    }
                }
                ExecutorKind::CloudRun => {
                    if task.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            name
                        )));
                    }
                    if task.job.as_deref().unwrap_or_default().is_empty() {
                        return Err(WorkflowValidationError::MissingTaskJob { task: name.clone() });
                    }
                }
                ExecutorKind::Library => {
                    if task.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            name
                        )));
                    }
                    if task.file.is_none() {
                        return Err(WorkflowValidationError::MissingTaskFile {
                            task: name.clone(),
                        });
                    }
                }
                ExecutorKind::Wasm => {
                    if task.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            name
                        )));
                    }
                    if task.file.is_none() {
                        return Err(WorkflowValidationError::MissingTaskFile {
                            task: name.clone(),
                        });
                    }
                }
                ExecutorKind::Custom(ref alias) => {
                    if task.custom_executor.is_none() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' references unknown custom executor '{}'",
                            name, alias
                        )));
                    }
                    if task.config_file.is_none() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' with custom executor '{}' requires 'config_file'",
                            name, alias
                        )));
                    }
                    if task.file.is_some()
                        || task.command.is_some()
                        || task.job.is_some()
                        || task.module.is_some()
                        || task.function.is_some()
                    {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' with custom executor '{}' cannot also set built-in executor fields like file, command, module, function, or job",
                            name, alias
                        )));
                    }
                }
            }

            if !task.inputs.is_null() && !task.inputs.is_object() {
                return Err(WorkflowValidationError::Custom(format!(
                    "task '{}' field 'inputs' must be an object",
                    name
                )));
            }
            if !task.env.is_null() {
                let Some(env_obj) = task.env.as_object() else {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' field 'env' must be an object",
                        name
                    )));
                };
                for (env_key, env_value) in env_obj {
                    validate_env_value(name, env_key, env_value)?;
                }
            }
            if !task.secrets.is_empty() {
                if !matches!(
                    task.executor,
                    ExecutorKind::Process | ExecutorKind::Python | ExecutorKind::Container
                ) {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' field 'secrets' is only supported for process, python, and container executors",
                        name
                    )));
                }
                let mut seen_names = std::collections::BTreeSet::new();
                let mut seen_paths = std::collections::BTreeSet::new();
                for mount in &task.secrets {
                    validate_secret_mount(name, mount)?;
                    if !seen_names.insert(mount.name.clone()) {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' secret mount name '{}' is duplicated",
                            name, mount.name
                        )));
                    }
                    let normalized_path = mount.path.to_string_lossy().to_string();
                    if !seen_paths.insert(normalized_path.clone()) {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' secret mount path '{}' is duplicated",
                            name, normalized_path
                        )));
                    }
                }
            }
        }

        if let Some(task) = detect_cycle(self) {
            return Err(WorkflowValidationError::Cycle { task });
        }

        self.validate_parallelism()?;
        if options.contract_validation {
            self.validate_types()?;
            self.validate_when_conditions()?;
        }
        self.validate_hooks()?;

        Ok(())
    }

    /// Validate workflow type declarations:
    /// - Any declared input/output schemas must be structurally valid
    /// - Tasks that define explicit `inputs` must declare `input_type`
    /// - All type schemas must use valid type expressions
    fn validate_types(&self) -> Result<(), WorkflowValidationError> {
        for (name, task) in &self.tasks {
            // Validate type schema structure
            if let Some(ref schema) = task.output_type {
                validate_type_schema(schema, name, "output_type", &self.types, &mut Vec::new())?;
            }
            if let Some(ref schema) = task.input_type {
                validate_type_schema(schema, name, "input_type", &self.types, &mut Vec::new())?;
            }
        }

        for (alias, schema) in &self.types {
            validate_type_schema(
                schema,
                "<types>",
                &format!("types.{}", alias),
                &self.types,
                &mut Vec::new(),
            )?;
        }

        // Enforce strict input binding: tasks with input_type must have matching inputs bindings
        for (name, task) in &self.tasks {
            // If task has input_type, validate its structure and require matching inputs
            if let Some(input_type) = task.input_type.as_ref() {
                let Some(type_obj) = input_type.as_object() else {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' input_type must be an object type (for named arguments)",
                        name
                    )));
                };

                // If input_type has any fields, inputs must be present and non-null
                if !type_obj.is_empty() {
                    if task.inputs.is_null() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' has input_type with fields but is missing 'inputs' bindings",
                            name
                        )));
                    }

                    let Some(bindings_obj) = task.inputs.as_object() else {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' inputs must be an object",
                            name
                        )));
                    };

                    // Check for missing bindings
                    for key in type_obj.keys() {
                        if !bindings_obj.contains_key(key) {
                            return Err(WorkflowValidationError::Custom(format!(
                                "task '{}' inputs missing binding for '{}'",
                                name, key
                            )));
                        }
                    }

                    // Check for extra bindings
                    for key in bindings_obj.keys() {
                        if !type_obj.contains_key(key) {
                            return Err(WorkflowValidationError::Custom(format!(
                                "task '{}' inputs contains unknown binding '{}'",
                                name, key
                            )));
                        }
                    }
                }
            }

            // When explicit `inputs` bindings are provided, validate their structure
            if !task.inputs.is_null() {
                let Some(bindings_obj) = task.inputs.as_object() else {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' inputs must be an object",
                        name
                    )));
                };

                if !bindings_obj.is_empty() && task.input_type.is_none() {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' defines non-empty 'inputs' but is missing 'input_type'",
                        name
                    )));
                }

                for (key, binding) in bindings_obj {
                    let Some(binding_obj) = binding.as_object() else {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' input binding '{}' must be an object with exactly one of: const, ref",
                            name, key
                        )));
                    };
                    let has_const = binding_obj.contains_key("const");
                    let has_ref = binding_obj.contains_key("ref");
                    if binding_obj.len() != 1 || has_const == has_ref {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' input binding '{}' must define exactly one of: const, ref",
                            name, key
                        )));
                    }
                    if has_ref {
                        let ref_path =
                            binding_obj
                                .get("ref")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    WorkflowValidationError::Custom(format!(
                                        "task '{}' input binding '{}.ref' must be a string",
                                        name, key
                                    ))
                                })?;
                        let parts: Vec<&str> = ref_path.split('.').collect();
                        let is_task_ref =
                            parts.len() >= 3 && parts[0] == "tasks" && parts[2] == "output";
                        let is_vars_ref = parts.len() >= 2 && parts[0] == "vars";
                        let is_parallel_ref = parts.as_slice() == ["parallel", "index"]
                            || parts.as_slice() == ["parallel", "count"];
                        let is_foreach_ref = parts.as_slice() == ["foreach", "index"]
                            || parts.as_slice() == ["foreach", "count"]
                            || (parts.len() >= 2 && parts[0] == "foreach" && parts[1] == "item");
                        if !(is_task_ref || is_vars_ref || is_parallel_ref || is_foreach_ref) {
                            return Err(WorkflowValidationError::Custom(format!(
                                "task '{}' input binding '{}' has invalid ref '{}'; expected tasks.<task>.output[.<field>...], vars.<NAME>, parallel.index, parallel.count, foreach.item[.<field>...], foreach.index, or foreach.count",
                                name, key, ref_path
                            )));
                        }
                        if is_task_ref {
                            let dep_task = parts[1];
                            if !task.depends_on.iter().any(|dep| dep == dep_task) {
                                return Err(WorkflowValidationError::Custom(format!(
                                    "task '{}' input binding '{}' ref '{}' targets '{}' which is not listed in depends_on",
                                    name, key, ref_path, dep_task
                                )));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate all `when` condition expressions in the workflow.
    ///
    /// For each task that has a `when:` field, this checks:
    /// 1. The expression parses successfully (syntax).
    /// 2. Every task referenced in the expression is listed in `depends_on`.
    /// 3. Every field path traversed on an output is consistent with the
    ///    declared `output_type` of the referenced task (when the type is known).
    fn validate_when_conditions(&self) -> Result<(), WorkflowValidationError> {
        for (name, task) in &self.tasks {
            let Some(ref when_str) = task.when else {
                continue;
            };

            // 1. Parse for syntax errors
            let expr = condition::parse_condition(when_str).map_err(|e| {
                WorkflowValidationError::Custom(format!(
                    "task '{}' has invalid 'when' expression: {}",
                    name, e
                ))
            })?;

            // 2. Every referenced task must be in depends_on
            let refs = condition::collect_task_refs(&expr);
            for (ref_task, fields) in &refs {
                if !task.depends_on.iter().any(|d| d == ref_task) {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' 'when' condition references task '{}' which is not listed in depends_on",
                        name, ref_task
                    )));
                }

                // 3. Validate field path against the referenced task's output_type (if declared)
                if let Some(ref_task_def) = self.tasks.get(*ref_task)
                    && let Some(ref output_type) = ref_task_def.output_type
                    && !fields.is_empty()
                {
                    validate_field_path(output_type, fields, ref_task, &self.types)
                                .map_err(|e| {
                                    WorkflowValidationError::Custom(format!(
                                        "task '{}' 'when' condition references invalid field path on task '{}': {}",
                                        name, ref_task, e
                                    ))
                                })?;
                }
            }
        }
        Ok(())
    }
}
