use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use crate::error::{CzarError, CzarResult, WorkflowValidationError};
use crate::python_runtime::resolve_python_execution_root;
use crate::workflow::{
    CustomExecutorDefinition, ExecutorKind, FailurePolicy, HookEvent, HookTaskFilter, ParallelMode,
    ParallelSpec, SecretFileMount, ValidationOptions, WasmMode, Workflow,
};

/// Interpolate `${{ vars.NAME }}` placeholders in a string.
///
/// The variable map is built from the workflow's `vars` block, with any
/// `CZAR_VAR_<NAME>` environment variables taking precedence.
fn interpolate(s: &str, vars: &BTreeMap<String, String>) -> String {
    let mut result = s.to_string();
    for (name, value) in vars {
        let vars_placeholder = format!("${{{{ vars.{name} }}}}");
        result = result.replace(&vars_placeholder, value);

        let built_in_placeholder = format!("${{{{ {name} }}}}");
        result = result.replace(&built_in_placeholder, value);
    }
    result
}

/// Platform-specific shared library extension without the leading dot.
#[cfg(target_os = "macos")]
const PLATFORM_LIB_EXT: &str = "dylib";
#[cfg(target_os = "windows")]
const PLATFORM_LIB_EXT: &str = "dll";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const PLATFORM_LIB_EXT: &str = "so";

/// Build the effective variable map: workflow vars overridden by CZAR_VAR_* env vars,
/// then explicit runtime overrides, plus read-only built-in platform variables.
fn effective_vars(
    workflow_vars: &BTreeMap<String, String>,
    var_overrides: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut vars = workflow_vars.clone();
    for (env_key, env_val) in std::env::vars() {
        if let Some(var_name) = env_key.strip_prefix("CZAR_VAR_") {
            vars.insert(var_name.to_string(), env_val);
        }
    }
    for (name, value) in var_overrides {
        vars.insert(name.clone(), value.clone());
    }
    // Built-in platform variables (not overrideable by user).
    // Inserted after env overrides so user cannot shadow them.
    vars.insert("platform.lib_ext".to_string(), PLATFORM_LIB_EXT.to_string());
    vars
}

/// If `path` has no extension, append the platform-specific shared library
/// extension (.so / .dylib / .dll).
fn resolve_library_extension(path: &Path) -> std::borrow::Cow<'_, Path> {
    if path.extension().is_some() {
        return std::borrow::Cow::Borrowed(path);
    }
    std::borrow::Cow::Owned(path.with_extension(PLATFORM_LIB_EXT))
}

/// Apply variable interpolation to an optional PathBuf.
fn interpolate_path(path: Option<&PathBuf>, vars: &BTreeMap<String, String>) -> Option<PathBuf> {
    path.map(|p| PathBuf::from(interpolate(&p.to_string_lossy(), vars)))
}

/// Apply variable interpolation to an optional String.
fn interpolate_opt_str(s: Option<&str>, vars: &BTreeMap<String, String>) -> Option<String> {
    s.map(|v| interpolate(v, vars))
}

/// Apply variable interpolation to a Vec<String>.
fn interpolate_args(args: &[String], vars: &BTreeMap<String, String>) -> Vec<String> {
    args.iter().map(|a| interpolate(a, vars)).collect()
}

fn interpolate_parallel_spec(
    spec: Option<&ParallelSpec>,
    vars: &BTreeMap<String, String>,
) -> Option<ParallelSpec> {
    match spec {
        Some(ParallelSpec::Count(count)) => Some(ParallelSpec::Count(*count)),
        Some(ParallelSpec::Ref(reference)) => Some(ParallelSpec::Ref(interpolate(reference, vars))),
        None => None,
    }
}

fn interpolate_secret_mounts(
    mounts: &[SecretFileMount],
    vars: &BTreeMap<String, String>,
) -> Vec<SecretFileMount> {
    mounts
        .iter()
        .map(|mount| SecretFileMount {
            name: interpolate(&mount.name, vars),
            secret: interpolate(&mount.secret, vars),
            path: PathBuf::from(interpolate(&mount.path.to_string_lossy(), vars)),
        })
        .collect()
}

/// Recursively interpolate all string leaves inside a JSON value.
fn interpolate_json(
    value: &serde_json::Value,
    vars: &BTreeMap<String, String>,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(interpolate(s, vars)),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| interpolate_json(item, vars))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), interpolate_json(item, vars)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

mod models;
mod python_contracts;
mod support;
mod task_rows;

pub use models::*;
#[cfg(test)]
use python_contracts::get_inspect_script;
use python_contracts::{introspect_python_signature, validate_python_task_type_contract};
use support::{resolve_declared_path, resolve_file, topo_sort};
pub use task_rows::build_workflow_tasks;
#[cfg(test)]
use task_rows::resolve_process_command;

pub fn inspect_python_task_signature(
    path: &Path,
    func_name: &str,
    project_root: &Path,
) -> Result<serde_json::Value, String> {
    introspect_python_signature(path, func_name, project_root)
}

impl Workflow {
    pub fn compile(&self, root: &Path) -> CzarResult<CompiledWorkflow> {
        self.compile_with_options(root, &BTreeMap::new(), ValidationOptions::STRICT)
    }

    pub fn compile_with_var_overrides(
        &self,
        root: &Path,
        var_overrides: &BTreeMap<String, String>,
    ) -> CzarResult<CompiledWorkflow> {
        self.compile_with_options(root, var_overrides, ValidationOptions::STRICT)
    }

    pub fn compile_with_options(
        &self,
        root: &Path,
        var_overrides: &BTreeMap<String, String>,
        validation_options: ValidationOptions,
    ) -> CzarResult<CompiledWorkflow> {
        // Build the effective variable map (workflow vars + CZAR_VAR_* env overrides
        // + explicit runtime overrides) used to interpolate runtime string fields
        // and string leaves inside JSON bindings.
        let vars = effective_vars(&self.vars, var_overrides);
        let workflow = self.interpolate_runtime_fields(&vars);

        // Validate the normalized workflow so vars-expanded strings participate in
        // the same validation path as literal workflow values.
        workflow
            .validate_with_options(validation_options)
            .map_err(CzarError::InvalidWorkflow)?;

        let mut name_index: IndexMap<String, usize> = IndexMap::new();
        for (idx, name) in workflow.tasks.keys().enumerate() {
            name_index.insert(name.clone(), idx);
        }

        let mut tasks = Vec::with_capacity(workflow.tasks.len());
        for (name, task) in &workflow.tasks {
            // Apply variable interpolation to path/command/args fields before resolution.
            let task_image_interp = interpolate_opt_str(task.image.as_deref(), &vars);
            let task_file_interp = interpolate_path(task.file.as_ref(), &vars);
            let task_command_interp = interpolate_opt_str(task.command.as_deref(), &vars);
            let task_args_interp = interpolate_args(&task.args, &vars);
            let task_job_interp = interpolate_opt_str(task.job.as_deref(), &vars);
            let task_module_interp = interpolate_opt_str(task.module.as_deref(), &vars);
            let task_function_interp = interpolate_opt_str(task.function.as_deref(), &vars);
            let task_when_interp = interpolate_opt_str(task.when.as_deref(), &vars);
            let task_parallel_interp = interpolate_parallel_spec(task.parallel.as_ref(), &vars);
            let task_foreach_interp = interpolate_opt_str(task.foreach.as_deref(), &vars);

            let mut signature = None;
            let task_config_file_interp = interpolate_path(task.config_file.as_ref(), &vars);
            let (file, config_file, python_path) = match task.executor {
                ExecutorKind::Container => (None, None, None),
                ExecutorKind::Python => {
                    let task_file = if let Some(file) = task_file_interp.as_ref() {
                        Some(resolve_file(root, name, file)?)
                    } else {
                        None
                    };
                    let context = format!("Python task {name}");
                    let python_path = resolve_python_execution_root(
                        None,
                        task_file.as_deref(),
                        Some(root),
                        &context,
                    )
                    .map_err(|e| CzarError::InvalidWorkflow(WorkflowValidationError::Custom(e)))?;

                    if let Some(path) = task_file.as_ref() {
                        let func_name = task_function_interp.as_deref().unwrap_or("main");
                        let python_signature =
                            introspect_python_signature(path, func_name, &python_path).map_err(
                                |e| CzarError::InvalidWorkflow(WorkflowValidationError::Custom(e)),
                            )?;
                        validate_python_task_type_contract(
                            name,
                            task,
                            func_name,
                            &python_signature,
                            &workflow.types,
                            validation_options,
                        )
                        .map_err(CzarError::InvalidWorkflow)?;
                        signature = Some(python_signature);
                    }
                    (task_file, None, Some(python_path))
                }
                ExecutorKind::Process => {
                    let file = task_file_interp
                        .as_ref()
                        .map(|file| resolve_declared_path(root, file));
                    (file, None, None)
                }
                ExecutorKind::Library => {
                    let file = task_file_interp
                        .as_ref()
                        .expect("validated library task file");
                    let file = resolve_library_extension(file);
                    (Some(resolve_declared_path(root, &file)), None, None)
                }
                ExecutorKind::Wasm => {
                    let file = task_file_interp.as_ref().expect("validated wasm task file");
                    (Some(resolve_declared_path(root, file)), None, None)
                }
                ExecutorKind::CloudRun => (None, None, None),
                ExecutorKind::Custom(_) => {
                    let config_file = task_config_file_interp
                        .as_ref()
                        .map(|file| resolve_file(root, name, file))
                        .transpose()?
                        .expect("validated custom executor config file");
                    let context = format!("Custom executor task {name}");
                    let python_path =
                        resolve_python_execution_root(None, None, Some(root), &context).map_err(
                            |e| CzarError::InvalidWorkflow(WorkflowValidationError::Custom(e)),
                        )?;
                    (None, Some(config_file), Some(python_path))
                }
            };
            let depends_on: Vec<usize> = task
                .depends_on
                .iter()
                .map(|d| *name_index.get(d).expect("validated dependency"))
                .collect();
            tasks.push(CompiledTask {
                name: name.clone(),
                executor: task.executor.clone(),
                image: task_image_interp,
                file,
                config_file,
                python_path,
                command: task_command_interp,
                args: task_args_interp,
                wasm_mode: task.wasm_mode.clone(),
                job: task_job_interp,
                module: task_module_interp,
                function: task_function_interp,
                custom_executor: task.custom_executor.clone(),
                inputs: interpolate_json(&task.inputs, &vars),
                env: interpolate_json(&task.env, &vars),
                secrets: interpolate_secret_mounts(&task.secrets, &vars),
                depends_on,
                timeout: task.timeout,
                retries: task.retries,
                retry_delay: task.retry_delay,
                on_failure: task.on_failure.clone(),
                signature,
                input_type: task.input_type.clone(),
                output_type: task.output_type.clone(),
                validate_output: validation_options.contract_validation && task.validate_output,
                when: task_when_interp,
                parallel: task_parallel_interp,
                foreach: task_foreach_interp,
                concurrency: task.concurrency,
                parallel_mode: task.parallel_mode.clone(),
            });
        }

        let topo = topo_sort(&tasks)?;

        // Compile hooks
        let mut hooks = Vec::new();
        for (hook_name, hook_def) in &workflow.hooks {
            // Apply variable interpolation to hook file/command fields.
            let hook_file_interp = interpolate_path(hook_def.file.as_ref(), &vars);
            let hook_command_interp = interpolate_opt_str(hook_def.command.as_deref(), &vars);
            let hook_job_interp = interpolate_opt_str(hook_def.job.as_deref(), &vars);
            let hook_module_interp = interpolate_opt_str(hook_def.module.as_deref(), &vars);
            let hook_function_interp = interpolate_opt_str(hook_def.function.as_deref(), &vars);

            let (file, python_path) = match hook_def.executor {
                ExecutorKind::Python => {
                    let hook_file = if let Some(file) = hook_file_interp.as_ref() {
                        Some(resolve_file(root, hook_name, file)?)
                    } else {
                        None
                    };
                    let context = format!("Python hook {hook_name}");
                    let python_path = resolve_python_execution_root(
                        None,
                        hook_file.as_deref(),
                        Some(root),
                        &context,
                    )
                    .map_err(|e| CzarError::InvalidWorkflow(WorkflowValidationError::Custom(e)))?;
                    (hook_file, Some(python_path))
                }
                ExecutorKind::Container => {
                    return Err(CzarError::InvalidWorkflow(WorkflowValidationError::Custom(
                        format!(
                            "hook '{}' cannot use executor 'container'; hooks currently support only built-in host and Cloud Run executors",
                            hook_name
                        ),
                    )));
                }
                ExecutorKind::Process => {
                    let file = hook_file_interp
                        .as_ref()
                        .map(|file| resolve_declared_path(root, file));
                    (file, None)
                }
                ExecutorKind::Library => {
                    let file = hook_file_interp
                        .as_ref()
                        .expect("validated library hook file");
                    let file = resolve_library_extension(file);
                    (Some(resolve_declared_path(root, &file)), None)
                }
                ExecutorKind::Wasm => {
                    let file = hook_file_interp.as_ref().expect("validated wasm hook file");
                    (Some(resolve_declared_path(root, file)), None)
                }
                ExecutorKind::CloudRun => (None, None),
                ExecutorKind::Custom(ref alias) => {
                    return Err(CzarError::InvalidWorkflow(WorkflowValidationError::Custom(
                        format!(
                            "hook '{}' cannot use custom executor '{}'; hooks currently support only built-in executors",
                            hook_name, alias
                        ),
                    )));
                }
            };

            hooks.push(CompiledHook {
                name: hook_name.clone(),
                event: hook_def.on.clone(),
                tasks_filter: hook_def.tasks.clone(),
                executor: hook_def.executor.clone(),
                file,
                python_path,
                command: hook_command_interp,
                job: hook_job_interp,
                module: hook_module_interp,
                function: hook_function_interp,
                timeout: hook_def.timeout,
            });
        }

        Ok(CompiledWorkflow {
            name: workflow.name.clone(),
            vars,
            types: workflow.types.clone(),
            tasks,
            name_index,
            topo,
            root: root.to_path_buf(),
            hooks,
        })
    }

    fn interpolate_runtime_fields(&self, vars: &BTreeMap<String, String>) -> Self {
        let mut workflow = self.clone();

        for task in workflow.tasks.values_mut() {
            task.image = interpolate_opt_str(task.image.as_deref(), vars);
            task.file = interpolate_path(task.file.as_ref(), vars);
            task.command = interpolate_opt_str(task.command.as_deref(), vars);
            task.args = interpolate_args(&task.args, vars);
            task.job = interpolate_opt_str(task.job.as_deref(), vars);
            task.module = interpolate_opt_str(task.module.as_deref(), vars);
            task.function = interpolate_opt_str(task.function.as_deref(), vars);
            task.inputs = interpolate_json(&task.inputs, vars);
            task.when = interpolate_opt_str(task.when.as_deref(), vars);
            task.parallel = interpolate_parallel_spec(task.parallel.as_ref(), vars);
            task.foreach = interpolate_opt_str(task.foreach.as_deref(), vars);
        }

        for hook in workflow.hooks.values_mut() {
            hook.file = interpolate_path(hook.file.as_ref(), vars);
            hook.command = interpolate_opt_str(hook.command.as_deref(), vars);
            hook.job = interpolate_opt_str(hook.job.as_deref(), vars);
            hook.module = interpolate_opt_str(hook.module.as_deref(), vars);
            hook.function = interpolate_opt_str(hook.function.as_deref(), vars);
        }

        workflow
    }
}

#[cfg(test)]
mod tests;
