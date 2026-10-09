use std::path::Path;

use crate::workflow::workflow_validation_helpers::expand_type_schema;
use crate::workflow::{ExecutorKind, HookTaskFilter, WasmMode};

use crate::compiled::{CompiledHook, CompiledTask, CompiledWorkflow};

pub(super) fn build_hook_params(
    hook: &CompiledHook,
    workflow_root: &Path,
) -> serde_json::Map<String, serde_json::Value> {
    let mut params = serde_json::Map::new();
    params.insert(
        "hook_event".to_string(),
        serde_json::Value::String(hook.event.as_str().to_string()),
    );
    params.insert(
        "hook_executor_type".to_string(),
        serde_json::Value::String(executor_type_str(&hook.executor).to_string()),
    );
    params.insert(
        "timeout_seconds".to_string(),
        serde_json::Value::Number(hook.timeout.into()),
    );

    let filter_value = match &hook.tasks_filter {
        None => serde_json::Value::Null,
        Some(HookTaskFilter::Star(_)) => serde_json::Value::Null,
        Some(HookTaskFilter::List(names)) => serde_json::json!(names),
    };
    params.insert("hook_tasks_filter".to_string(), filter_value);

    match &hook.executor {
        ExecutorKind::Python => {
            if let Some(file) = hook.file.as_ref() {
                params.insert(
                    "task_file".to_string(),
                    serde_json::Value::String(file.to_string_lossy().to_string()),
                );
            }
            if let Some(module) = hook.module.as_deref() {
                params.insert(
                    "task_module".to_string(),
                    serde_json::Value::String(module.to_string()),
                );
            }
            if let Some(function) = hook.function.as_deref() {
                params.insert(
                    "task_function".to_string(),
                    serde_json::Value::String(function.to_string()),
                );
            }
            if let Some(python_path) = hook.python_path.as_ref() {
                params.insert(
                    "python_path".to_string(),
                    serde_json::Value::String(python_path.to_string_lossy().to_string()),
                );
            }
        }
        ExecutorKind::Container => {
            unreachable!("validated hooks reject container executor");
        }
        ExecutorKind::Process => {
            if let Some(command) = hook.command.as_deref() {
                params.insert(
                    "command".to_string(),
                    serde_json::Value::String(resolve_process_command(command, workflow_root)),
                );
            } else if let Some(file) = hook.file.as_ref() {
                params.insert(
                    "command".to_string(),
                    serde_json::Value::String(file.to_string_lossy().to_string()),
                );
            }
        }
        ExecutorKind::CloudRun => {
            if let Some(job) = hook.job.as_deref() {
                params.insert(
                    "job_name".to_string(),
                    serde_json::Value::String(job.to_string()),
                );
            }
        }
        ExecutorKind::Library => {
            if let Some(file) = hook.file.as_ref() {
                params.insert(
                    "library_path".to_string(),
                    serde_json::Value::String(file.to_string_lossy().to_string()),
                );
            }
            if let Some(function) = hook.function.as_deref() {
                params.insert(
                    "library_function".to_string(),
                    serde_json::Value::String(function.to_string()),
                );
            }
        }
        ExecutorKind::Wasm => {
            if let Some(file) = hook.file.as_ref() {
                params.insert(
                    "wasm_module_path".to_string(),
                    serde_json::Value::String(file.to_string_lossy().to_string()),
                );
            }
            if let Some(function) = hook.function.as_deref() {
                params.insert(
                    "wasm_entrypoint".to_string(),
                    serde_json::Value::String(function.to_string()),
                );
            }
        }
        ExecutorKind::Custom(alias) => {
            unreachable!("validated hooks reject custom executor '{}'", alias);
        }
    }

    params
}

pub(super) fn executor_type_str(executor: &ExecutorKind) -> String {
    match executor {
        ExecutorKind::Container => "container".to_string(),
        ExecutorKind::CloudRun => "cloudrun".to_string(),
        ExecutorKind::Process => "process".to_string(),
        ExecutorKind::Python => "python".to_string(),
        ExecutorKind::Library => "library".to_string(),
        ExecutorKind::Wasm => "wasm".to_string(),
        ExecutorKind::Custom(alias) => format!("custom:{alias}"),
    }
}

pub(super) fn build_task_signature(
    task: &CompiledTask,
    compiled: &CompiledWorkflow,
) -> Option<serde_json::Value> {
    let mut signature = task.signature.clone().unwrap_or(serde_json::json!({}));
    if let Some(input_type) = task.input_type.as_ref() {
        signature["input_type"] = expand_type_schema(input_type, &compiled.types)
            .expect("compiled workflow input_type must already be valid");
    }
    if task.validate_output
        && let Some(output_type) = task.output_type.as_ref()
    {
        signature["output_type"] = expand_type_schema(output_type, &compiled.types)
            .expect("compiled workflow output_type must already be valid");
    }
    if (task.input_type.is_some() || task.output_type.is_some()) && !compiled.types.is_empty() {
        signature["type_aliases"] = serde_json::json!(compiled.types);
    }
    if signature == serde_json::json!({}) {
        None
    } else {
        Some(signature)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn resolve_process_command(command: &str, root: &Path) -> String {
    if command.chars().any(char::is_whitespace) {
        return command.to_string();
    }

    let path = Path::new(command);
    if !path.is_relative() || !command.contains('/') {
        return maybe_append_windows_exe(path).to_string_lossy().to_string();
    }

    maybe_append_windows_exe(&root.join(path))
        .to_string_lossy()
        .to_string()
}

fn maybe_append_windows_exe(path: &Path) -> std::borrow::Cow<'_, Path> {
    #[cfg(windows)]
    {
        if path.extension().is_none() {
            return std::borrow::Cow::Owned(path.with_extension("exe"));
        }
    }
    std::borrow::Cow::Borrowed(path)
}

pub(super) fn wasm_mode_str(mode: &WasmMode) -> &'static str {
    match mode {
        WasmMode::Embedded => "embedded",
        WasmMode::Cli => "cli",
    }
}
