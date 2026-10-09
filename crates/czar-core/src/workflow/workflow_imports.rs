use std::path::Path;

use super::*;
#[path = "workflow_imports_support.rs"]
mod support;
use support::*;

#[derive(Debug, Default, Deserialize)]
struct WorkflowProjectConfig {
    #[serde(default)]
    executors: BTreeMap<String, CustomExecutorDefinition>,
}

impl WorkflowDefinition {
    pub fn parse_yaml_with_root(
        data: &str,
        workflow_root: &Path,
    ) -> Result<Self, WorkflowValidationError> {
        let mut parsed = Self::parse_yaml(data).map_err(|err| {
            WorkflowValidationError::Custom(format!("failed to parse workflow YAML: {err}"))
        })?;
        parsed.executor_registry = load_project_executor_registry(workflow_root)?;
        parsed.expand_imports(workflow_root)
    }

    pub fn expand_imports(&self, workflow_root: &Path) -> Result<Self, WorkflowValidationError> {
        if self.imports.is_empty() {
            for (task_name, template) in &self.tasks {
                if template.executor.is_none() {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' is missing required field 'executor'",
                        task_name
                    )));
                }
            }
            return Ok(self.clone());
        }

        let mut generated_tasks = IndexMap::new();
        let mut generated_groups = BTreeMap::new();
        let mut generated_flows = IndexMap::new();

        for import in &self.imports {
            let generated = expand_dbt_import(import, workflow_root)?;
            merge_generated_tasks(&mut generated_tasks, generated.tasks)?;
            merge_generated_groups(&mut generated_groups, generated.task_groups)?;
            merge_generated_flows(&mut generated_flows, generated.flows)?;
        }

        let mut merged_tasks = generated_tasks;
        for (task_name, override_task) in &self.tasks {
            if let Some(base_task) = merged_tasks.get(task_name).cloned() {
                if override_task.executor.is_some() {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' is imported from dbt and cannot override its executor; only depends_on, timeout, retries, retry_delay, on_failure, and when are supported",
                        task_name
                    )));
                }
                ensure_supported_import_override(task_name, override_task)?;
                merged_tasks.insert(
                    task_name.clone(),
                    merge_imported_task_override(base_task, override_task),
                );
            } else {
                if override_task.executor.is_none() {
                    return Err(WorkflowValidationError::Custom(format!(
                        "task '{}' is missing required field 'executor'",
                        task_name
                    )));
                }
                merged_tasks.insert(task_name.clone(), override_task.clone());
            }
        }

        let mut merged_task_groups = generated_groups;
        for (group_name, members) in &self.task_groups {
            if merged_task_groups.contains_key(group_name) {
                return Err(WorkflowValidationError::Custom(format!(
                    "workflow.task_groups.{} conflicts with an auto-generated dbt task group",
                    group_name
                )));
            }
            merged_task_groups.insert(group_name.clone(), members.clone());
        }

        let mut merged_flows = generated_flows;
        for (flow_name, flow) in &self.flows {
            if merged_flows.contains_key(flow_name) {
                return Err(WorkflowValidationError::Custom(format!(
                    "workflow.flows.{} conflicts with an auto-generated dbt flow",
                    flow_name
                )));
            }
            merged_flows.insert(flow_name.clone(), flow.clone());
        }

        Ok(Self {
            name: self.name.clone(),
            description: self.description.clone(),
            schedule: self.schedule.clone(),
            types: self.types.clone(),
            vars: self.vars.clone(),
            imports: Vec::new(),
            task_groups: merged_task_groups,
            tasks: merged_tasks,
            flows: merged_flows,
            hooks: self.hooks.clone(),
            executor_registry: self.executor_registry.clone(),
        })
    }
}

fn load_project_executor_registry(
    workflow_root: &Path,
) -> Result<BTreeMap<String, CustomExecutorDefinition>, WorkflowValidationError> {
    let Some(config_path) = workflow_root
        .ancestors()
        .map(|dir| dir.join("czar.yaml"))
        .find(|path| path.is_file())
    else {
        return Ok(BTreeMap::new());
    };

    let config_content = std::fs::read_to_string(&config_path).map_err(|err| {
        WorkflowValidationError::Custom(format!(
            "failed to read project config '{}': {}",
            config_path.display(),
            err
        ))
    })?;
    let config: WorkflowProjectConfig = serde_yaml::from_str(&config_content).map_err(|err| {
        WorkflowValidationError::Custom(format!(
            "failed to parse project config '{}': {}",
            config_path.display(),
            err
        ))
    })?;

    for (alias, executor) in &config.executors {
        if alias.trim().is_empty() {
            return Err(WorkflowValidationError::Custom(format!(
                "project config '{}' contains an executor with an empty alias",
                config_path.display()
            )));
        }
        if matches!(
            alias.trim().to_ascii_lowercase().as_str(),
            "process" | "python" | "cloudrun" | "library" | "wasm"
        ) {
            return Err(WorkflowValidationError::Custom(format!(
                "project config '{}' cannot redefine built-in executor alias '{}'",
                config_path.display(),
                alias
            )));
        }
        if !executor.target.contains(':') {
            return Err(WorkflowValidationError::Custom(format!(
                "project config '{}' executor '{}' must use a Python class target like 'package.module:ClassName'",
                config_path.display(),
                alias
            )));
        }
    }

    Ok(config.executors)
}

fn merge_generated_tasks(
    existing: &mut IndexMap<String, TaskTemplateDefinition>,
    incoming: IndexMap<String, TaskTemplateDefinition>,
) -> Result<(), WorkflowValidationError> {
    for (task_name, task) in incoming {
        if existing.contains_key(&task_name) {
            return Err(WorkflowValidationError::Custom(format!(
                "dbt import generated duplicate task '{}'",
                task_name
            )));
        }
        existing.insert(task_name, task);
    }
    Ok(())
}

fn merge_generated_groups(
    existing: &mut BTreeMap<String, Vec<String>>,
    incoming: BTreeMap<String, Vec<String>>,
) -> Result<(), WorkflowValidationError> {
    for (group_name, tasks) in incoming {
        if existing.contains_key(&group_name) {
            return Err(WorkflowValidationError::Custom(format!(
                "dbt import generated duplicate task group '{}'",
                group_name
            )));
        }
        existing.insert(group_name, tasks);
    }
    Ok(())
}

fn merge_generated_flows(
    existing: &mut IndexMap<String, FlowDefinition>,
    incoming: IndexMap<String, FlowDefinition>,
) -> Result<(), WorkflowValidationError> {
    for (flow_name, flow) in incoming {
        if existing.contains_key(&flow_name) {
            return Err(WorkflowValidationError::Custom(format!(
                "dbt import generated duplicate flow '{}'",
                flow_name
            )));
        }
        existing.insert(flow_name, flow);
    }
    Ok(())
}

fn ensure_supported_import_override(
    task_name: &str,
    template: &TaskTemplateDefinition,
) -> Result<(), WorkflowValidationError> {
    let has_unsupported_fields = template.file.is_some()
        || template.config_file.is_some()
        || template.command.is_some()
        || !template.args.is_empty()
        || template.job.is_some()
        || template.module.is_some()
        || template.function.is_some()
        || template.custom_executor.is_some()
        || !template.inputs.is_null()
        || !template.env.is_null()
        || !template.secrets.is_empty()
        || template.input_type.is_some()
        || template.output_type.is_some()
        || template.parallel.is_some()
        || template.foreach.is_some()
        || template.concurrency.is_some()
        || template.parallel_mode != ParallelMode::default();
    if has_unsupported_fields {
        return Err(WorkflowValidationError::Custom(format!(
            "task '{}' is an imported dbt task override; only depends_on, timeout, retries, retry_delay, on_failure, and when are supported",
            task_name
        )));
    }
    Ok(())
}

fn merge_imported_task_override(
    mut base: TaskTemplateDefinition,
    override_task: &TaskTemplateDefinition,
) -> TaskTemplateDefinition {
    let mut combined = base.depends_on.clone();
    let mut seen = BTreeSet::new();
    for dep in &combined {
        seen.insert(dep.clone());
    }
    for dep in &override_task.depends_on {
        if seen.insert(dep.clone()) {
            combined.push(dep.clone());
        }
    }
    base.depends_on = combined;
    if override_task.timeout != default_timeout_secs() {
        base.timeout = override_task.timeout;
    }
    if override_task.retries != default_retries() {
        base.retries = override_task.retries;
    }
    if override_task.retry_delay.is_some() {
        base.retry_delay = override_task.retry_delay;
    }
    if override_task.on_failure != FailurePolicy::default() {
        base.on_failure = override_task.on_failure.clone();
    }
    if override_task.when.is_some() {
        base.when = override_task.when.clone();
    }
    base
}

fn expand_dbt_import(
    import: &DbtImportDefinition,
    workflow_root: &Path,
) -> Result<GeneratedDbtImport, WorkflowValidationError> {
    let project_dir = resolve_import_path(workflow_root, &import.dbt);
    let manifest_path = ensure_dbt_manifest(import, workflow_root, &project_dir)?;
    let manifest = load_dbt_manifest(&manifest_path)?;
    let label = sanitize_segment(
        import
            .label
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                if manifest.metadata.project_name.trim().is_empty() {
                    project_dir
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("dbt")
                } else {
                    manifest.metadata.project_name.as_str()
                }
            }),
    );
    let mut generated_nodes = Vec::new();
    let mut unique_to_task = HashMap::new();

    for node in manifest.nodes.values() {
        if !is_supported_dbt_node(node, import) {
            continue;
        }
        if !matches_selectors(node, import) {
            continue;
        }
        let generated = generate_dbt_task(node, &label, workflow_root, &project_dir, import);
        unique_to_task.insert(node.unique_id.clone(), generated.task_name.clone());
        generated_nodes.push((node.clone(), generated));
    }

    let imported_unique_ids: BTreeSet<String> = unique_to_task.keys().cloned().collect();
    let mut tasks = IndexMap::new();
    let mut task_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut flows: IndexMap<String, FlowDefinition> = IndexMap::new();
    let mut all_tasks = Vec::new();

    for (node, mut generated) in generated_nodes {
        let imported_depends_on: Vec<String> = node
            .depends_on
            .nodes
            .iter()
            .filter(|dep| imported_unique_ids.contains(*dep))
            .filter_map(|dep| unique_to_task.get(dep).cloned())
            .collect();
        if imported_depends_on.is_empty() {
            for dep in &import.depends_on {
                if !generated.depends_on.contains(dep) {
                    generated.depends_on.push(dep.clone());
                }
            }
        } else {
            generated.depends_on = imported_depends_on;
        }

        let task_name = generated.task_name.clone();
        all_tasks.push(task_name.clone());
        push_group_member(
            &mut task_groups,
            &format!("dbt.{}.all", label),
            task_name.clone(),
        );
        push_group_member(
            &mut task_groups,
            &format!("dbt.{}.resource.{}", label, generated.resource_group),
            task_name.clone(),
        );
        for path_group in &generated.group_paths {
            push_group_member(&mut task_groups, path_group, task_name.clone());
        }

        let mut template = generated.template;
        template.depends_on = generated.depends_on;
        tasks.insert(task_name, template);
    }

    if !all_tasks.is_empty() {
        flows.insert(
            format!("dbt.{}.all", label),
            FlowDefinition {
                description: Some(format!("All imported dbt tasks for '{}'", label)),
                tasks: FlowTaskSet::List(all_tasks),
            },
        );
    }
    for (group_name, members) in &task_groups {
        flows.insert(
            group_name.clone(),
            FlowDefinition {
                description: Some(format!("Auto-generated dbt selection for '{}'", group_name)),
                tasks: FlowTaskSet::List(members.clone()),
            },
        );
    }

    Ok(GeneratedDbtImport {
        tasks,
        task_groups,
        flows,
    })
}
