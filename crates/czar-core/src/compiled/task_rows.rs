use crate::database::NewWorkflowTask;
use crate::workflow::FailurePolicy;
use crate::workflow::{ExecutorKind, ParallelMode, ParallelSpec};

use super::{CompiledTask, CompiledWorkflow};

mod support;

#[cfg(test)]
pub(crate) use support::resolve_process_command;
use support::{
    build_hook_params, build_task_signature, executor_type_str,
    resolve_process_command as resolve_process_command_impl, wasm_mode_str,
};

/// Build workflow tasks from a compiled workflow.
///
/// Converts a `CompiledWorkflow` into `NewWorkflowTask` rows for the database.
///
/// **Static parallel expansion** (`parallel: N`, `parallel_mode: czar`):
/// Expands a task into N instances named `task[0]`, `task[1]`, ..., `task[N-1]`.
/// Each instance gets `parallel_index` and `parallel_count` in its params.
/// Downstream `depends_on` entries referencing the original task name are rewritten
/// to reference all expanded instances.
///
/// **Cloud Run native parallelism** (`parallel: N`, `parallel_mode: cloud_run_tasks`):
/// Stores `task_count: N` in the task params and emits a single task.
/// The Cloud Run executor passes this to the `task_count` field of the run request.
///
/// Dynamic parallelism (`parallel: discover.count`, `foreach: discover.files`) is
/// stored in params for runtime expansion by the scheduler.
pub fn build_workflow_tasks(compiled: &CompiledWorkflow) -> Vec<NewWorkflowTask> {
    // First pass: compute which tasks expand statically in Czar mode.
    // Maps original task name -> list of expanded instance names.
    let mut expansion_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for task in &compiled.tasks {
        if let Some(ParallelSpec::Count(n)) = &task.parallel
            && task.parallel_mode == ParallelMode::Czar
            && *n > 1
        {
            let names = (0..*n as usize)
                .map(|i| format!("{}[{}]", task.name, i))
                .collect();
            expansion_map.insert(task.name.clone(), names);
        }
    }

    let mut tasks = Vec::new();
    let mut task_index = 0i32;

    for task in &compiled.tasks {
        let orig_deps: Vec<String> = task
            .depends_on
            .iter()
            .filter_map(|dep_idx| compiled.tasks.get(*dep_idx).map(|t| t.name.clone()))
            .collect();
        let expanded_deps: Vec<String> = orig_deps
            .iter()
            .flat_map(|dep| {
                expansion_map
                    .get(dep)
                    .cloned()
                    .unwrap_or_else(|| vec![dep.clone()])
            })
            .collect();

        let executor_type = executor_type_str(&task.executor);
        let base_params = build_task_params(task, compiled);
        let signature = build_task_signature(task, compiled);

        if let Some(ParallelSpec::Count(n)) = &task.parallel
            && task.parallel_mode == ParallelMode::Czar
            && *n > 1
        {
            let count = *n;
            for i in 0..count as usize {
                let mut params = base_params.clone();
                params.insert(
                    "parallel_index".to_string(),
                    serde_json::Value::Number(i.into()),
                );
                params.insert(
                    "parallel_count".to_string(),
                    serde_json::Value::Number(count.into()),
                );
                if let Some(c) = task.concurrency {
                    params.insert(
                        "parallel_concurrency".to_string(),
                        serde_json::Value::Number(c.into()),
                    );
                }
                params.insert(
                    "parallel_group".to_string(),
                    serde_json::Value::String(task.name.clone()),
                );
                tasks.push(NewWorkflowTask {
                    task_index,
                    task_name: format!("{}[{}]", task.name, i),
                    executor_type: executor_type.clone(),
                    depends_on: expanded_deps.clone(),
                    params: serde_json::Value::Object(params),
                    signature: signature.clone(),
                });
                task_index += 1;
            }
            continue;
        }

        let mut params = base_params;
        if let Some(ParallelSpec::Count(n)) = &task.parallel
            && task.parallel_mode == ParallelMode::CloudRunTasks
        {
            params.insert(
                "task_count".to_string(),
                serde_json::Value::Number((*n).into()),
            );
        }

        tasks.push(NewWorkflowTask {
            task_index,
            task_name: task.name.clone(),
            executor_type,
            depends_on: expanded_deps,
            params: serde_json::Value::Object(params),
            signature,
        });
        task_index += 1;
    }

    for hook in &compiled.hooks {
        let params = build_hook_params(hook, &compiled.root);

        tasks.push(NewWorkflowTask {
            task_index,
            task_name: format!("__hook__:{}", hook.name),
            executor_type: "hook".to_string(),
            depends_on: vec![],
            params: serde_json::Value::Object(params),
            signature: None,
        });
        task_index += 1;
    }

    tasks
}
fn build_task_params(
    task: &CompiledTask,
    compiled: &CompiledWorkflow,
) -> serde_json::Map<String, serde_json::Value> {
    let mut params = serde_json::Map::new();
    if !compiled.vars.is_empty() {
        params.insert(
            "workflow_vars".to_string(),
            serde_json::json!(compiled.vars),
        );
    }
    if !task.inputs.is_null() {
        params.insert("task_bindings".to_string(), task.inputs.clone());
    }
    if !task.env.is_null() {
        params.insert("env".to_string(), task.env.clone());
    }
    if !task.secrets.is_empty() {
        params.insert("secrets".to_string(), serde_json::json!(task.secrets));
    }
    if let Some(when) = task.when.as_ref() {
        params.insert(
            "condition_expr".to_string(),
            serde_json::Value::String(when.clone()),
        );
    }
    if let Some(ParallelSpec::Ref(reference)) = task.parallel.as_ref() {
        params.insert(
            "parallel_ref".to_string(),
            serde_json::Value::String(reference.clone()),
        );
        params.insert(
            "parallel_group".to_string(),
            serde_json::Value::String(task.name.clone()),
        );
    }
    if let Some(reference) = task.foreach.as_ref() {
        params.insert(
            "foreach_ref".to_string(),
            serde_json::Value::String(reference.clone()),
        );
        params.insert(
            "parallel_group".to_string(),
            serde_json::Value::String(task.name.clone()),
        );
    }
    if task.parallel.is_some() || task.foreach.is_some() {
        params.insert(
            "parallel_mode".to_string(),
            serde_json::Value::String(
                match task.parallel_mode {
                    ParallelMode::Czar => "czar",
                    ParallelMode::CloudRunTasks => "cloud_run_tasks",
                }
                .to_string(),
            ),
        );
    }
    if let Some(concurrency) = task.concurrency {
        params.insert(
            "parallel_concurrency".to_string(),
            serde_json::Value::Number(concurrency.into()),
        );
    }
    params.insert(
        "max_retries".to_string(),
        serde_json::Value::Number(task.retries.into()),
    );
    if let Some(retry_delay) = task.retry_delay {
        params.insert(
            "retry_delay_seconds".to_string(),
            serde_json::Value::Number(retry_delay.into()),
        );
    }
    if task.on_failure != FailurePolicy::default() {
        params.insert(
            "on_failure".to_string(),
            serde_json::Value::String(
                match task.on_failure {
                    FailurePolicy::Skip => "skip",
                    FailurePolicy::Run => "run",
                }
                .to_string(),
            ),
        );
    }
    params.insert(
        "timeout_seconds".to_string(),
        serde_json::Value::Number(task.timeout.into()),
    );

    match &task.executor {
        ExecutorKind::Container => {
            if let Some(image) = task.image.as_deref() {
                params.insert(
                    "container_image".to_string(),
                    serde_json::Value::String(image.to_string()),
                );
            }
            params.insert(
                "command".to_string(),
                serde_json::Value::String(task.command.clone().unwrap_or_default()),
            );
            if !task.args.is_empty() {
                params.insert(
                    "task_args".to_string(),
                    serde_json::Value::Array(
                        task.args
                            .iter()
                            .map(|a| serde_json::Value::String(a.clone()))
                            .collect(),
                    ),
                );
            }
        }
        ExecutorKind::CloudRun => {
            if let Some(job) = task.job.as_deref() {
                params.insert(
                    "job_name".to_string(),
                    serde_json::Value::String(job.to_string()),
                );
            }
        }
        ExecutorKind::Process => {
            if let Some(command) = task.command.as_deref() {
                params.insert(
                    "command".to_string(),
                    serde_json::Value::String(resolve_process_command_impl(
                        command,
                        &compiled.root,
                    )),
                );
            } else if let Some(file) = task.file.as_ref() {
                params.insert(
                    "command".to_string(),
                    serde_json::Value::String(file.to_string_lossy().to_string()),
                );
            }
            if !task.args.is_empty() {
                params.insert(
                    "task_args".to_string(),
                    serde_json::Value::Array(
                        task.args
                            .iter()
                            .map(|a| serde_json::Value::String(a.clone()))
                            .collect(),
                    ),
                );
            }
        }
        ExecutorKind::Python => {
            if let Some(file) = task.file.as_ref() {
                params.insert(
                    "task_file".to_string(),
                    serde_json::Value::String(file.to_string_lossy().to_string()),
                );
            }
            if let Some(module) = task.module.as_deref() {
                params.insert(
                    "task_module".to_string(),
                    serde_json::Value::String(module.to_string()),
                );
            }
            if let Some(function) = task.function.as_deref() {
                params.insert(
                    "task_function".to_string(),
                    serde_json::Value::String(function.to_string()),
                );
            }
            let python_path = task
                .python_path
                .as_ref()
                .expect("compiled python task must include python_path");
            params.insert(
                "python_path".to_string(),
                serde_json::Value::String(python_path.to_string_lossy().to_string()),
            );
        }
        ExecutorKind::Custom(alias) => {
            let custom_executor = task
                .custom_executor
                .as_ref()
                .expect("compiled custom executor task must include custom executor metadata");
            let config_file = task
                .config_file
                .as_ref()
                .expect("compiled custom executor task must include config_file");
            let python_path = task
                .python_path
                .as_ref()
                .expect("compiled custom executor task must include python_path");
            params.insert(
                "custom_executor_alias".to_string(),
                serde_json::Value::String(alias.clone()),
            );
            params.insert(
                "custom_executor".to_string(),
                serde_json::json!(custom_executor),
            );
            params.insert(
                "config_file".to_string(),
                serde_json::Value::String(config_file.to_string_lossy().to_string()),
            );
            params.insert(
                "python_path".to_string(),
                serde_json::Value::String(python_path.to_string_lossy().to_string()),
            );
        }
        ExecutorKind::Library => {
            if let Some(file) = task.file.as_ref() {
                params.insert(
                    "library_path".to_string(),
                    serde_json::Value::String(file.to_string_lossy().to_string()),
                );
            }
            if let Some(function) = task.function.as_deref() {
                params.insert(
                    "library_function".to_string(),
                    serde_json::Value::String(function.to_string()),
                );
            }
        }
        ExecutorKind::Wasm => {
            if let Some(file) = task.file.as_ref() {
                params.insert(
                    "wasm_module_path".to_string(),
                    serde_json::Value::String(file.to_string_lossy().to_string()),
                );
            }
            if let Some(function) = task.function.as_deref() {
                params.insert(
                    "wasm_entrypoint".to_string(),
                    serde_json::Value::String(function.to_string()),
                );
            }
            if !task.args.is_empty() {
                params.insert(
                    "task_args".to_string(),
                    serde_json::Value::Array(
                        task.args
                            .iter()
                            .map(|a| serde_json::Value::String(a.clone()))
                            .collect(),
                    ),
                );
            }
            params.insert(
                "wasm_mode".to_string(),
                serde_json::Value::String(wasm_mode_str(&task.wasm_mode).to_string()),
            );
        }
    }

    params
}
