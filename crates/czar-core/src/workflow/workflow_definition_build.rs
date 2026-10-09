use super::*;

impl WorkflowDefinition {
    pub(super) fn build_all_tasks_workflow(&self) -> Result<Workflow, WorkflowValidationError> {
        let mut tasks = IndexMap::new();
        for (task_name, template) in &self.tasks {
            tasks.insert(
                task_name.clone(),
                self.build_task_definition(task_name, template, None)?,
            );
        }
        Ok(Workflow {
            name: self.name.clone(),
            description: self.description.clone(),
            schedule: self.schedule.clone(),
            types: self.types.clone(),
            vars: self.vars.clone(),
            tasks,
            hooks: self.hooks.clone(),
        })
    }

    pub(super) fn expand_flow_tasks(
        &self,
        flow_name: &str,
        flow: &FlowDefinition,
    ) -> Result<IndexMap<String, FlowTaskDefinition>, WorkflowValidationError> {
        let mut tasks = IndexMap::new();
        match &flow.tasks {
            FlowTaskSet::Map(map) => {
                for (task_name, task_entry) in map {
                    let task = match task_entry {
                        FlowTaskEntry::Config(task) => task.clone(),
                        FlowTaskEntry::Empty(()) => FlowTaskDefinition::default(),
                    };
                    tasks.insert(task_name.clone(), task);
                }
            }
            FlowTaskSet::List(list) => {
                let mut include_all = false;
                for task_name in list {
                    let task_name = task_name.trim();
                    if task_name.is_empty() {
                        return Err(WorkflowValidationError::Custom(format!(
                            "workflow.flows.{}.tasks contains an empty task name",
                            flow_name
                        )));
                    }
                    if task_name == "*" {
                        include_all = true;
                        continue;
                    }
                    tasks.entry(task_name.to_string()).or_default();
                }
                if include_all {
                    for task_name in self.tasks.keys() {
                        tasks.entry(task_name.clone()).or_default();
                    }
                }
            }
            FlowTaskSet::Star(value) => {
                if value.trim() != "*" {
                    return Err(WorkflowValidationError::Custom(format!(
                        "workflow.flows.{}.tasks string shorthand must be '*'",
                        flow_name
                    )));
                }
                for task_name in self.tasks.keys() {
                    tasks.insert(task_name.clone(), FlowTaskDefinition::default());
                }
            }
        }
        Ok(tasks)
    }

    pub(super) fn build_flow_workflow(
        &self,
        flow_name: &str,
    ) -> Result<Workflow, WorkflowValidationError> {
        let flow = self.flows.get(flow_name).ok_or_else(|| {
            WorkflowValidationError::Custom(format!("unknown flow '{}'", flow_name))
        })?;
        let flow_tasks = self.expand_flow_tasks(flow_name, flow)?;
        if flow_tasks.is_empty() {
            return Err(WorkflowValidationError::Custom(format!(
                "workflow.flows.{} must contain at least one task",
                flow_name
            )));
        }

        for (task_name, flow_task) in &flow_tasks {
            if !self.tasks.contains_key(task_name) {
                return Err(WorkflowValidationError::Custom(format!(
                    "workflow.flows.{}.tasks references unknown task '{}'",
                    flow_name, task_name
                )));
            }
            if !flow_task.depends_on.is_empty() {
                return Err(WorkflowValidationError::Custom(format!(
                    "workflow.flows.{}.tasks.{}.depends_on is no longer supported; move dependencies to workflow.tasks.{}.depends_on",
                    flow_name, task_name, task_name
                )));
            }
            if let Some(inputs) = flow_task.inputs.as_ref()
                && !inputs.is_null()
                && !inputs.is_object()
            {
                return Err(WorkflowValidationError::Custom(format!(
                    "workflow.flows.{}.tasks.{}.inputs must be an object",
                    flow_name, task_name
                )));
            }
        }

        let required = self.collect_flow_dependencies(flow_name, &flow_tasks)?;
        let mut tasks = IndexMap::new();
        for (task_name, template) in &self.tasks {
            if !required.contains(task_name) {
                continue;
            }
            let flow_task = flow_tasks.get(task_name);
            tasks.insert(
                task_name.clone(),
                self.build_task_definition(task_name, template, flow_task)?,
            );
        }

        Ok(Workflow {
            name: self.name.clone(),
            description: self.description.clone(),
            schedule: self.schedule.clone(),
            types: self.types.clone(),
            vars: self.vars.clone(),
            tasks,
            hooks: self.hooks.clone(),
        })
    }

    fn collect_flow_dependencies(
        &self,
        flow_name: &str,
        flow_tasks: &IndexMap<String, FlowTaskDefinition>,
    ) -> Result<BTreeSet<String>, WorkflowValidationError> {
        let mut required = BTreeSet::new();
        let mut stack: Vec<String> = flow_tasks.keys().cloned().collect();
        while let Some(task_name) = stack.pop() {
            if !required.insert(task_name.clone()) {
                continue;
            }
            let template = self.tasks.get(&task_name).ok_or_else(|| {
                WorkflowValidationError::Custom(format!(
                    "workflow.flows.{}.tasks references unknown task '{}'",
                    flow_name, task_name
                ))
            })?;
            let mut seen_deps = BTreeSet::new();
            for dep in &template.depends_on {
                if dep.trim().is_empty() {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}': dependency name cannot be empty",
                        task_name
                    )));
                }
                if dep == &task_name {
                    return Err(WorkflowValidationError::SelfDependency {
                        task: task_name.clone(),
                    });
                }
                if !self.tasks.contains_key(dep) {
                    return Err(WorkflowValidationError::UnknownDependency {
                        task: task_name.clone(),
                        dependency: dep.clone(),
                    });
                }
                if !seen_deps.insert(dep.clone()) {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' has duplicate dependency '{}'",
                        task_name, dep
                    )));
                }
                stack.push(dep.clone());
            }
        }
        Ok(required)
    }

    fn build_task_definition(
        &self,
        task_name: &str,
        template: &TaskTemplateDefinition,
        flow_task: Option<&FlowTaskDefinition>,
    ) -> Result<TaskDefinition, WorkflowValidationError> {
        let inputs = flow_task
            .and_then(|task| task.inputs.clone())
            .unwrap_or_else(|| template.inputs.clone());
        if !inputs.is_null() && !inputs.is_object() {
            return Err(WorkflowValidationError::Custom(format!(
                "task '{}' field 'inputs' must be an object",
                task_name
            )));
        }

        Ok(TaskDefinition {
            executor: template.executor.clone().expect("validated task executor"),
            image: template.image.clone(),
            file: template.file.clone(),
            config_file: template.config_file.clone(),
            command: template.command.clone(),
            args: template.args.clone(),
            wasm_mode: template.wasm_mode.clone(),
            job: template.job.clone(),
            module: template.module.clone(),
            function: template.function.clone(),
            custom_executor: template
                .executor
                .as_ref()
                .and_then(ExecutorKind::custom_alias)
                .and_then(|alias| self.executor_registry.get(alias).cloned()),
            inputs,
            env: flow_task
                .and_then(|task| task.env.clone())
                .unwrap_or_else(|| template.env.clone()),
            secrets: template.secrets.clone(),
            depends_on: template.depends_on.clone(),
            timeout: flow_task
                .and_then(|task| task.timeout)
                .unwrap_or(template.timeout),
            retries: flow_task
                .and_then(|task| task.retries)
                .unwrap_or(template.retries),
            retry_delay: flow_task
                .and_then(|task| task.retry_delay)
                .or(template.retry_delay),
            on_failure: flow_task
                .and_then(|task| task.on_failure.clone())
                .unwrap_or_else(|| template.on_failure.clone()),
            input_type: template.input_type.clone(),
            output_type: template.output_type.clone(),
            validate_output: template.validate_output,
            when: template.when.clone(),
            parallel: template.parallel.clone(),
            foreach: template.foreach.clone(),
            concurrency: template.concurrency,
            parallel_mode: template.parallel_mode.clone(),
        })
    }
}
