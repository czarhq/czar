use super::*;

impl WorkflowDefinition {
    pub fn parse_yaml(data: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(data)
    }

    pub fn flow_names(&self) -> Vec<String> {
        self.flows.keys().cloned().collect()
    }

    pub fn selected_flow_name(
        &self,
        requested_flow: Option<&str>,
    ) -> Result<String, WorkflowValidationError> {
        if let Some(flow_name) = requested_flow {
            let flow_name = flow_name.trim();
            if flow_name.is_empty() {
                return Err(WorkflowValidationError::Custom(
                    "flow name cannot be empty".to_string(),
                ));
            }
            if self.flows.contains_key(flow_name) {
                return Ok(flow_name.to_string());
            }
            return Err(WorkflowValidationError::Custom(format!(
                "unknown flow '{}'",
                flow_name
            )));
        }

        Ok(ALL_TASKS_FLOW.to_string())
    }

    pub fn expand_group_tasks(
        &self,
        group_names: &[String],
    ) -> Result<Vec<String>, WorkflowValidationError> {
        let mut expanded = BTreeSet::new();
        for group_name in group_names {
            let Some(tasks) = self.task_groups.get(group_name) else {
                return Err(WorkflowValidationError::Custom(format!(
                    "unknown task group '{}'",
                    group_name
                )));
            };
            for task_name in tasks {
                expanded.insert(task_name.clone());
            }
        }
        Ok(expanded.into_iter().collect())
    }

    pub fn resolve_flow(
        &self,
        requested_flow: Option<&str>,
    ) -> Result<Workflow, WorkflowValidationError> {
        self.resolve_flow_with_options(requested_flow, ValidationOptions::STRICT)
    }

    pub fn resolve_flow_with_options(
        &self,
        requested_flow: Option<&str>,
        options: ValidationOptions,
    ) -> Result<Workflow, WorkflowValidationError> {
        self.validate_catalog_with_options(options)?;
        let flow_name = self.selected_flow_name(requested_flow)?;
        if flow_name == ALL_TASKS_FLOW {
            self.build_all_tasks_workflow()
        } else {
            self.build_flow_workflow(&flow_name)
        }
    }
}
