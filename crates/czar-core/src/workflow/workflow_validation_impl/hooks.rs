use super::super::*;

impl Workflow {
    pub(super) fn validate_hooks(&self) -> Result<(), WorkflowValidationError> {
        use crate::workflow::HookTaskFilter;

        for (hook_name, hook) in &self.hooks {
            if hook_name.trim().is_empty() {
                return Err(WorkflowValidationError::Custom(
                    "hook name cannot be empty".to_string(),
                ));
            }

            // Validate executor requirements (same rules as tasks)
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
                ExecutorKind::Library => {
                    if hook.file.is_none() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "hook '{}' with executor 'library' requires 'file'",
                            hook_name
                        )));
                    }
                }
                ExecutorKind::Wasm => {
                    if hook.file.is_none() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "hook '{}' with executor 'wasm' requires 'file'",
                            hook_name
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

            // tasks filter is only meaningful for task-level events
            if let Some(filter) = &hook.tasks {
                if hook.on.is_run_event() {
                    return Err(WorkflowValidationError::Custom(format!(
                        "hook '{}' has 'tasks' filter but '{}' is a run-level event; 'tasks' filter is only valid for task-level events",
                        hook_name,
                        hook.on.as_str()
                    )));
                }
                // Note: task name existence is validated in WorkflowDefinition::validate_catalog
                // against the full task set (all flows). Here we only check the filter format.
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
