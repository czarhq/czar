use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

use crate::compiled::CompiledWorkflow;
use crate::python_runtime::{require_python_project_root, uv_python_command};
use crate::workflow::WorkflowDefinition;

/// Load, parse, validate, and compile a workflow from a path in one step.
///
/// This is the canonical entry point used by the worker: it owns the full
/// pipeline (source → YAML → definition → compiled workflow) in one place so
/// callers don't have to chain the steps themselves.  It is intentionally
/// self-contained rather than delegating to a higher-level orchestrator path,
/// because the worker compiles workflows offline without a database.
pub fn compile_workflow_from_path(
    path: &Path,
) -> Result<(crate::workflow::Workflow, CompiledWorkflow)> {
    let content = load_workflow_source(path)?;
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let definition =
        WorkflowDefinition::parse_yaml_with_root(&content, &root).map_err(anyhow::Error::msg)?;
    definition.validate_catalog().map_err(anyhow::Error::msg)?;
    let workflow = definition.resolve_flow(None).map_err(anyhow::Error::msg)?;
    let compiled = workflow.compile(&root).map_err(anyhow::Error::msg)?;
    Ok((workflow, compiled))
}

/// Load workflow source text from a path.
///
/// Supported inputs:
/// - `.yaml` / `.yml`: read file contents directly, unless it is a dynamic
///   source descriptor (`generate:`), in which case execute the generator and
///   read its output file.
/// - `.py`: unsupported as a direct source; wrap it in a YAML descriptor.
pub fn load_workflow_source(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    match ext.as_deref() {
        Some("yaml") | Some("yml") => load_from_yaml(path),
        _ => Err(anyhow!(
            "Workflow source must be a YAML file (.yaml/.yml): {}",
            path.display()
        )),
    }
}

#[derive(Debug, Deserialize, Default)]
struct WorkflowSourceDescriptor {
    #[serde(default)]
    generate: Option<WorkflowGeneratorSpec>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Deserialize, Default)]
struct WorkflowGeneratorSpec {
    kind: String,
    script: PathBuf,
    #[serde(default)]
    args: Vec<serde_yaml::Value>,
    #[serde(default)]
    output: Option<PathBuf>,
    #[serde(default)]
    refresh: bool,
}

fn load_from_yaml(path: &Path) -> Result<String> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read workflow source: {}", path.display()))?;
    let Some(generate) = parse_generator_descriptor(path, &raw)? else {
        return Ok(raw);
    };
    let output_path = resolve_output_path(path, generate.output.as_deref())?;

    if !generate.refresh && output_path.exists() {
        return read_generated_output(&output_path);
    }

    let script_path = resolve_script_path(path, &generate.script)?;
    match generate.kind.as_str() {
        "python" => run_python_generator(&script_path, &generate.args, &output_path),
        "native" => run_native_generator(&script_path, &generate.args, &output_path),
        other => Err(anyhow!(
            "Unsupported workflow generator kind '{}' in {} (supported: python, native)",
            other,
            path.display()
        )),
    }
}

fn parse_generator_descriptor(path: &Path, raw: &str) -> Result<Option<WorkflowGeneratorSpec>> {
    let parsed: serde_yaml::Value = match serde_yaml::from_str(raw) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    let has_generate = parsed
        .as_mapping()
        .map(|mapping| mapping.contains_key(serde_yaml::Value::String("generate".to_string())))
        .unwrap_or(false);
    if !has_generate {
        return Ok(None);
    }

    let descriptor: WorkflowSourceDescriptor =
        serde_yaml::from_value(parsed).with_context(|| {
            format!(
                "Failed to parse workflow source as YAML: {}",
                path.display()
            )
        })?;
    let Some(generate) = descriptor.generate else {
        return Ok(None);
    };
    if !descriptor.extra.is_empty() {
        return Err(anyhow!(
            "Dynamic workflow descriptor {} must only contain the 'generate' key",
            path.display()
        ));
    }
    Ok(Some(generate))
}

fn resolve_script_path(source_path: &Path, script: &Path) -> Result<PathBuf> {
    if script.is_absolute() {
        return Ok(script.to_path_buf());
    }
    let base = source_path.parent().unwrap_or_else(|| Path::new("."));
    Ok(base.join(script))
}

fn resolve_output_path(source_path: &Path, output: Option<&Path>) -> Result<PathBuf> {
    let base = source_path.parent().unwrap_or_else(|| Path::new("."));
    let base = if base.is_absolute() {
        base.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(base)
    };
    let resolved = match output {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => base.join(path),
        None => default_cached_output_path(source_path)?,
    };
    Ok(resolved)
}

fn default_cached_output_path(source_path: &Path) -> Result<PathBuf> {
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            anyhow!(
                "Invalid workflow descriptor path (missing file stem): {}",
                source_path.display()
            )
        })?;
    let dir = source_path.parent().unwrap_or_else(|| Path::new("."));
    Ok(dir.join(format!("{stem}.generated.yaml")))
}

fn read_generated_output(path: &Path) -> Result<String> {
    let generated = std::fs::read_to_string(path).with_context(|| {
        format!(
            "Failed to read generated workflow output file: {}",
            path.display()
        )
    })?;
    if generated.trim().is_empty() {
        return Err(anyhow!(
            "Workflow generator produced empty output file: {}",
            path.display()
        ));
    }
    Ok(generated)
}

fn run_python_generator(
    script_path: &Path,
    args: &[serde_yaml::Value],
    output_path: &Path,
) -> Result<String> {
    let project_root = require_python_project_root(script_path, "Python workflow generator")
        .map_err(anyhow::Error::msg)?;
    let mut command = uv_python_command(&project_root);
    command.arg(script_path);
    run_generator(command, script_path, args, output_path)
}

fn run_native_generator(
    script_path: &Path,
    args: &[serde_yaml::Value],
    output_path: &Path,
) -> Result<String> {
    let parent = script_path.parent().unwrap_or_else(|| Path::new("."));
    let mut command = Command::new(script_path);
    command.current_dir(parent);
    run_generator(command, script_path, args, output_path)
}

/// Run a workflow generator script and read its output.
fn run_generator(
    mut command: Command,
    script_path: &Path,
    args: &[serde_yaml::Value],
    output_path: &Path,
) -> Result<String> {
    if let Some(parent_dir) = output_path.parent() {
        std::fs::create_dir_all(parent_dir).with_context(|| {
            format!(
                "Failed to create workflow generator output directory: {}",
                parent_dir.display()
            )
        })?;
    }

    command.arg("--output");
    command.arg(output_path);
    for arg in args {
        let s = match arg {
            serde_yaml::Value::String(s) => s.clone(),
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::Bool(b) => b.to_string(),
            other => return Err(anyhow!("Unsupported generator arg type: {:?}", other)),
        };
        command.arg(s);
    }

    let display = format!("{command:?}");
    let output = command.output().with_context(|| {
        format!(
            "Failed to execute workflow generator '{}': {}",
            display,
            script_path.display()
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            format!("workflow generator failed: {}", script_path.display())
        } else {
            format!(
                "workflow generator failed: {}: {}",
                script_path.display(),
                stderr
            )
        };
        return Err(anyhow!(message));
    }

    if !output_path.exists() {
        return Err(anyhow!(
            "Workflow generator did not create output file: {}",
            output_path.display()
        ));
    }

    read_generated_output(output_path)
}

#[cfg(test)]
mod tests;
