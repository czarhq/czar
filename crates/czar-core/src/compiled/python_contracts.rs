use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::WorkflowValidationError;
use crate::python_runtime::{require_explicit_python_project_root, uv_python_command};
use crate::workflow::ValidationOptions;
use crate::workflow::workflow_validation_helpers::expand_type_schema;

/// Embedded Python introspection script
const INSPECT_TASK_PY: &str = include_str!("runtime/inspect_task.py");

pub(super) fn get_inspect_script() -> std::result::Result<PathBuf, String> {
    use std::fs;

    use sha2::{Digest, Sha256};

    let cache_dir = std::env::temp_dir().join("czar").join("runtime");

    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create runtime cache dir: {}", e))?;

    let mut hasher = Sha256::new();
    hasher.update(INSPECT_TASK_PY.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let script_path = cache_dir.join(format!("inspect_task-{}.py", &hash[..12]));

    let should_write = match fs::read_to_string(&script_path) {
        Ok(existing) => existing != INSPECT_TASK_PY,
        Err(_) => true,
    };
    if should_write {
        fs::write(&script_path, INSPECT_TASK_PY)
            .map_err(|e| format!("Failed to write introspection script: {}", e))?;
    }

    Ok(script_path)
}

pub(super) fn introspect_python_signature(
    path: &Path,
    func_name: &str,
    project_root: &Path,
) -> std::result::Result<serde_json::Value, String> {
    let script_path = get_inspect_script()?;
    introspect_python_signature_with_script(path, func_name, project_root, &script_path)
}

pub(super) fn introspect_python_signature_with_script(
    path: &Path,
    func_name: &str,
    project_root: &Path,
    script_path: &Path,
) -> std::result::Result<serde_json::Value, String> {
    let project_root =
        require_explicit_python_project_root(project_root, "Python task introspection")?;
    let runner_label = format!("uv run python (project: {})", project_root.display());
    let mut cmd = uv_python_command(&project_root);
    cmd.arg(script_path).arg(path).arg(func_name);
    let output = cmd.output().map_err(|err| {
        format!(
            "Failed to execute introspection script via {}: {}",
            runner_label, err
        )
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = serde_json::from_str::<serde_json::Value>(&stdout);
    if let Ok(value) = parsed.as_ref()
        && let Some(err) = value.get("error").and_then(|v| v.as_str())
    {
        return Err(format!(
            "Task analysis error ({} -> {}): {}",
            path.display(),
            func_name,
            err
        ));
    }

    if output.status.success() {
        let value = if let Ok(value) = parsed {
            value
        } else {
            let parse_err = parsed.expect_err("parse should fail in this branch");
            return Err(format!(
                "Failed to parse introspection output: {}",
                parse_err
            ));
        };
        return Ok(value);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "Introspection failed via {} ({}): {}",
        runner_label,
        func_name,
        stderr.trim()
    ))
}

pub(super) fn validate_python_task_type_contract(
    task_name: &str,
    task: &crate::workflow::TaskDefinition,
    function_name: &str,
    signature: &serde_json::Value,
    aliases: &BTreeMap<String, serde_json::Value>,
    validation_options: ValidationOptions,
) -> Result<(), WorkflowValidationError> {
    if !validation_options.contract_validation {
        return Ok(());
    }

    if let Some(input_type) = task.input_type.as_ref() {
        let declared = expand_type_schema(input_type, aliases).map_err(|reason| {
            WorkflowValidationError::Custom(format!(
                "task '{}' input_type is invalid: {}",
                task_name, reason
            ))
        })?;
        let actual = signature
            .get("inputs")
            .ok_or_else(|| {
                WorkflowValidationError::Custom(format!(
                    "task '{}' Python function '{}' did not report parameter types",
                    task_name, function_name
                ))
            })?
            .clone();

        if declared != actual {
            return Err(WorkflowValidationError::Custom(format!(
                "task '{}' input_type does not match Python function '{}' parameters: declared {} but function reports {}",
                task_name,
                function_name,
                serde_json::to_string(&declared).unwrap_or_else(|_| "<invalid>".to_string()),
                serde_json::to_string(&actual).unwrap_or_else(|_| "<invalid>".to_string())
            )));
        }
    }

    if task.validate_output
        && let Some(output_type) = task.output_type.as_ref()
    {
        let declared = expand_type_schema(output_type, aliases).map_err(|reason| {
            WorkflowValidationError::Custom(format!(
                "task '{}' output_type is invalid: {}",
                task_name, reason
            ))
        })?;
        let actual = signature.get("output").ok_or_else(|| {
            WorkflowValidationError::Custom(format!(
                "task '{}' Python function '{}' did not report a return annotation",
                task_name, function_name
            ))
        })?;
        if actual.is_null() {
            return Err(WorkflowValidationError::Custom(format!(
                "task '{}' declares output_type but Python function '{}' is missing a return annotation",
                task_name, function_name
            )));
        }

        if declared != *actual {
            return Err(WorkflowValidationError::Custom(format!(
                "task '{}' output_type does not match Python function '{}' return annotation: declared {} but function reports {}",
                task_name,
                function_name,
                serde_json::to_string(&declared).unwrap_or_else(|_| "<invalid>".to_string()),
                serde_json::to_string(actual).unwrap_or_else(|_| "<invalid>".to_string())
            )));
        }
    }

    Ok(())
}
