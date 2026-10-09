use super::*;

impl WorkflowDefinition {
    pub fn validate_catalog(&self) -> Result<(), WorkflowValidationError> {
        self.validate_catalog_with_options(ValidationOptions::STRICT)
    }

    pub fn validate_catalog_with_options(
        &self,
        options: ValidationOptions,
    ) -> Result<(), WorkflowValidationError> {
        if self.name.trim().is_empty() {
            return Err(WorkflowValidationError::EmptyWorkflowName);
        }
        if self.tasks.is_empty() {
            return Err(WorkflowValidationError::NoTasks);
        }

        for (task_name, template) in &self.tasks {
            if task_name.trim().is_empty() {
                return Err(WorkflowValidationError::EmptyTaskName);
            }
            let executor = template.executor.as_ref().ok_or_else(|| {
                WorkflowValidationError::Custom(format!(
                    "task '{}' is missing required field 'executor'",
                    task_name
                ))
            })?;
            if !template.inputs.is_null() && !template.inputs.is_object() {
                return Err(WorkflowValidationError::Custom(format!(
                    "task '{}' field 'inputs' must be an object",
                    task_name
                )));
            }
            match executor {
                ExecutorKind::Container => {
                    if template.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            task_name
                        )));
                    }
                    if template
                        .image
                        .as_deref()
                        .unwrap_or_default()
                        .trim()
                        .is_empty()
                    {
                        return Err(WorkflowValidationError::MissingTaskImage {
                            task: task_name.clone(),
                        });
                    }
                    if template.file.is_some()
                        || template.module.is_some()
                        || template.function.is_some()
                        || template.job.is_some()
                    {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' with executor 'container' cannot set file, module, function, or job",
                            task_name
                        )));
                    }
                }
                ExecutorKind::Python => {
                    if template.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            task_name
                        )));
                    }
                    let has_module = template
                        .module
                        .as_deref()
                        .map(|module| !module.is_empty())
                        .unwrap_or(false);
                    if template.file.is_none() && !has_module {
                        return Err(WorkflowValidationError::MissingTaskFile {
                            task: task_name.clone(),
                        });
                    }
                }
                ExecutorKind::Process => {
                    if template.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            task_name
                        )));
                    }
                    if template.command.is_none() && template.file.is_none() {
                        return Err(WorkflowValidationError::MissingTaskCommand {
                            task: task_name.clone(),
                        });
                    }
                }
                ExecutorKind::CloudRun => {
                    if template.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            task_name
                        )));
                    }
                    if template.job.as_deref().unwrap_or_default().is_empty() {
                        return Err(WorkflowValidationError::MissingTaskJob {
                            task: task_name.clone(),
                        });
                    }
                }
                ExecutorKind::Library | ExecutorKind::Wasm => {
                    if template.config_file.is_some() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' field 'config_file' is only supported for custom executors",
                            task_name
                        )));
                    }
                    if template.file.is_none() {
                        return Err(WorkflowValidationError::MissingTaskFile {
                            task: task_name.clone(),
                        });
                    }
                }
                ExecutorKind::Custom(alias) => {
                    if !self.executor_registry.contains_key(alias) {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' references unknown custom executor '{}'; define it in czar.yaml executors",
                            task_name, alias
                        )));
                    }
                    if template.config_file.is_none() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' with custom executor '{}' requires 'config_file'",
                            task_name, alias
                        )));
                    }
                    if template.file.is_some()
                        || template.command.is_some()
                        || template.job.is_some()
                        || template.module.is_some()
                        || template.function.is_some()
                    {
                        return Err(WorkflowValidationError::Custom(format!(
                            "task '{}' with custom executor '{}' cannot also set built-in executor fields like file, command, module, function, or job",
                            task_name, alias
                        )));
                    }
                }
            }
        }

        for (group_name, members) in &self.task_groups {
            if group_name.trim().is_empty() {
                return Err(WorkflowValidationError::Custom(
                    "workflow.task_groups group name cannot be empty".to_string(),
                ));
            }
            if members.is_empty() {
                return Err(WorkflowValidationError::Custom(format!(
                    "workflow.task_groups.{} must contain at least one task",
                    group_name
                )));
            }
            let mut seen = BTreeSet::new();
            for member in members {
                let member = member.trim();
                if member.is_empty() {
                    return Err(WorkflowValidationError::Custom(format!(
                        "workflow.task_groups.{} contains an empty task name",
                        group_name
                    )));
                }
                if !self.tasks.contains_key(member) {
                    return Err(WorkflowValidationError::Custom(format!(
                        "workflow.task_groups.{} references unknown task '{}'",
                        group_name, member
                    )));
                }
                if !seen.insert(member.to_string()) {
                    return Err(WorkflowValidationError::Custom(format!(
                        "workflow.task_groups.{} has duplicate task '{}'",
                        group_name, member
                    )));
                }
            }
        }

        self.validate_catalog_hooks()?;

        let all_workflow = self.build_all_tasks_workflow()?;
        all_workflow.validate_with_options(options)?;

        for flow_name in self.flows.keys() {
            let flow_workflow = self.build_flow_workflow(flow_name)?;
            flow_workflow.validate_with_options(options)?;
        }

        Ok(())
    }

    fn validate_catalog_hooks(&self) -> Result<(), WorkflowValidationError> {
        use crate::workflow::{ExecutorKind, HookTaskFilter};

        for (hook_name, hook) in &self.hooks {
            if hook_name.trim().is_empty() {
                return Err(WorkflowValidationError::Custom(
                    "hook name cannot be empty".to_string(),
                ));
            }

            match hook.executor {
                ExecutorKind::Python => {
                    let has_module = hook
                        .module
                        .as_deref()
                        .map(|m| !m.is_empty())
                        .unwrap_or(false);
                    if hook.file.is_none() && !has_module {
                        return Err(WorkflowValidationError::Custom(format!(
                            "hook '{}' with executor 'python' requires 'module' or 'file'",
                            hook_name
                        )));
                    }
                }
                ExecutorKind::Container => {
                    return Err(WorkflowValidationError::Custom(format!(
                        "hook '{}' cannot use executor 'container'; hooks currently support only built-in host and Cloud Run executors",
                        hook_name
                    )));
                }
                ExecutorKind::Process => {
                    if hook.command.is_none() && hook.file.is_none() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "hook '{}' with executor 'process' requires 'command' or 'file'",
                            hook_name
                        )));
                    }
                }
                ExecutorKind::CloudRun => {
                    if hook.job.as_deref().unwrap_or_default().is_empty() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "hook '{}' with executor 'cloudrun' requires 'job'",
                            hook_name
                        )));
                    }
                }
                ExecutorKind::Library | ExecutorKind::Wasm => {
                    if hook.file.is_none() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "hook '{}' with executor '{}' requires 'file'",
                            hook_name,
                            match hook.executor {
                                ExecutorKind::Library => "library",
                                _ => "wasm",
                            }
                        )));
                    }
                }
                ExecutorKind::Custom(ref alias) => {
                    return Err(WorkflowValidationError::Custom(format!(
                        "hook '{}' cannot use custom executor '{}'; hooks currently support only built-in executors",
                        hook_name, alias
                    )));
                }
            }

            if let Some(filter) = &hook.tasks {
                if hook.on.is_run_event() {
                    return Err(WorkflowValidationError::Custom(format!(
                        "hook '{}' has 'tasks' filter but '{}' is a run-level event; \
                         'tasks' filter is only valid for task-level events",
                        hook_name,
                        hook.on.as_str()
                    )));
                }
                if let HookTaskFilter::List(names) = filter {
                    for task_name in names {
                        if !self.tasks.contains_key(task_name) {
                            return Err(WorkflowValidationError::Custom(format!(
                                "hook '{}' tasks filter references unknown task '{}'",
                                hook_name, task_name
                            )));
                        }
                    }
                }
                if let HookTaskFilter::Star(s) = filter
                    && s != "*"
                {
                    return Err(WorkflowValidationError::Custom(format!(
                        "hook '{}' tasks filter string shorthand must be '*'",
                        hook_name
                    )));
                }
            }
        }

        Ok(())
    }
}
