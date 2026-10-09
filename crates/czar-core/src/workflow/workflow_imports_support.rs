use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::*;

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct DbtManifest {
    #[serde(default)]
    pub(super) metadata: DbtManifestMetadata,
    #[serde(default)]
    pub(super) nodes: BTreeMap<String, DbtManifestNode>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct DbtManifestMetadata {
    #[serde(default)]
    pub(super) project_name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct DbtManifestNode {
    pub(super) unique_id: String,
    pub(super) name: String,
    pub(super) resource_type: String,
    #[serde(default)]
    pub(super) path: String,
    #[serde(default)]
    pub(super) original_file_path: String,
    #[serde(default)]
    pub(super) fqn: Vec<String>,
    #[serde(default)]
    pub(super) depends_on: DbtManifestDependsOn,
    #[serde(default)]
    pub(super) config: DbtManifestConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct DbtManifestDependsOn {
    #[serde(default)]
    pub(super) nodes: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct DbtManifestConfig {
    #[serde(default)]
    pub(super) materialized: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct GeneratedDbtImport {
    pub(super) tasks: IndexMap<String, TaskTemplateDefinition>,
    pub(super) task_groups: BTreeMap<String, Vec<String>>,
    pub(super) flows: IndexMap<String, FlowDefinition>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct GeneratedDbtTask {
    pub(super) task_name: String,
    pub(super) group_paths: Vec<String>,
    pub(super) resource_group: String,
    pub(super) depends_on: Vec<String>,
    pub(super) template: TaskTemplateDefinition,
}

pub(super) fn push_group_member(
    groups: &mut BTreeMap<String, Vec<String>>,
    group: &str,
    task_name: String,
) {
    let entry = groups.entry(group.to_string()).or_default();
    if !entry.iter().any(|existing| existing == &task_name) {
        entry.push(task_name);
    }
}

pub(super) fn resolve_import_path(workflow_root: &Path, path: &Path) -> PathBuf {
    absolute(&workflow_root.join(path))
}

/// `path` from the current folder ("" is the current folder itself).
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

pub(super) fn ensure_dbt_manifest(
    import: &DbtImportDefinition,
    workflow_root: &Path,
    project_dir: &Path,
) -> Result<PathBuf, WorkflowValidationError> {
    let manifest_path = project_dir.join("target").join("manifest.json");
    if manifest_path.exists() {
        return Ok(manifest_path);
    }

    let mut command = build_dbt_parse_command(workflow_root, project_dir)?;
    command.arg("parse");
    command.arg("--project-dir").arg(project_dir);
    if let Some(profiles_dir) = import.profiles_dir.as_ref() {
        command
            .arg("--profiles-dir")
            .arg(resolve_import_path(workflow_root, profiles_dir));
    }
    if let Some(profile) = import.profile.as_deref() {
        command.arg("--profile").arg(profile);
    }
    if let Some(target) = import.target.as_deref() {
        command.arg("--target").arg(target);
    }

    let output = command.output().map_err(|err| {
        WorkflowValidationError::Custom(format!(
            "failed to execute 'dbt parse' for import '{}': {}",
            project_dir.display(),
            err
        ))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "dbt parse failed".to_string()
        };
        return Err(WorkflowValidationError::Custom(format!(
            "dbt parse failed for '{}': {}",
            project_dir.display(),
            details
        )));
    }
    if !manifest_path.exists() {
        return Err(WorkflowValidationError::Custom(format!(
            "dbt parse did not produce manifest.json under '{}'",
            project_dir.display()
        )));
    }
    Ok(manifest_path)
}

fn build_dbt_parse_command(
    workflow_root: &Path,
    project_dir: &Path,
) -> Result<Command, WorkflowValidationError> {
    // dbt runs inside these folders, where a relative path such as bin/dbt
    // would point somewhere else.
    let workflow_root = &absolute(workflow_root);
    let project_dir = &absolute(project_dir);
    let project_local = project_dir.join("bin").join("dbt");
    if project_local.is_file() {
        let mut command = Command::new(project_local);
        command.current_dir(project_dir);
        return Ok(command);
    }
    let workflow_local = workflow_root.join("bin").join("dbt");
    if workflow_local.is_file() {
        let mut command = Command::new(workflow_local);
        command.current_dir(workflow_root);
        return Ok(command);
    }
    if project_dir.join("pyproject.toml").exists() {
        let mut command = Command::new("uv");
        command.current_dir(project_dir);
        command.arg("run").arg("dbt");
        return Ok(command);
    }
    if workflow_root.join("pyproject.toml").exists() {
        let mut command = Command::new("uv");
        command.current_dir(workflow_root);
        command.arg("run").arg("dbt");
        return Ok(command);
    }
    let mut command = Command::new("dbt");
    command.current_dir(project_dir);
    Ok(command)
}

pub(super) fn load_dbt_manifest(path: &Path) -> Result<DbtManifest, WorkflowValidationError> {
    let raw = fs::read_to_string(path).map_err(|err| {
        WorkflowValidationError::Custom(format!(
            "failed to read dbt manifest '{}': {}",
            path.display(),
            err
        ))
    })?;
    serde_json::from_str(&raw).map_err(|err| {
        WorkflowValidationError::Custom(format!(
            "failed to parse dbt manifest '{}': {}",
            path.display(),
            err
        ))
    })
}

pub(super) fn is_supported_dbt_node(node: &DbtManifestNode, import: &DbtImportDefinition) -> bool {
    match node.resource_type.as_str() {
        "model" => {
            import.include_models
                && !matches!(
                    node.config.materialized.as_deref(),
                    Some("ephemeral") | Some("skip")
                )
        }
        "test" => import.include_tests,
        "seed" => import.include_seeds,
        "snapshot" => import.include_snapshots,
        _ => false,
    }
}

pub(super) fn matches_selectors(node: &DbtManifestNode, import: &DbtImportDefinition) -> bool {
    let mut selectors = import.select.clone();
    selectors.extend(import.models.clone());
    if selectors.is_empty() {
        return true;
    }

    let fqn_no_project = if node.fqn.len() > 1 {
        node.fqn[1..].join(".")
    } else {
        node.name.clone()
    };
    let path_no_ext = relative_path_without_extension(node);
    let candidates = [
        fqn_no_project.as_str(),
        path_no_ext.as_str(),
        node.name.as_str(),
        node.unique_id.as_str(),
    ];

    selectors.iter().any(|selector| {
        let normalized = selector
            .strip_prefix("path:")
            .map(|value| value.replace('/', "."))
            .unwrap_or_else(|| selector.clone());
        candidates
            .iter()
            .any(|candidate| wildcard_match(&normalized, candidate))
    })
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == value;
    }
    let mut remaining = value;
    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');

    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if idx == 0 && anchored_start {
            if let Some(rest) = remaining.strip_prefix(part) {
                remaining = rest;
                continue;
            }
            return false;
        }
        if idx == parts.len() - 1 && anchored_end {
            return remaining.ends_with(part);
        }
        if let Some(pos) = remaining.find(part) {
            remaining = &remaining[pos + part.len()..];
        } else {
            return false;
        }
    }

    true
}

pub(super) fn generate_dbt_task(
    node: &DbtManifestNode,
    label: &str,
    workflow_root: &Path,
    project_dir: &Path,
    import: &DbtImportDefinition,
) -> GeneratedDbtTask {
    let task_name = format!("dbt.{}.{}", label, build_relative_dbt_name(node));
    let resource_group = pluralize_resource_type(&node.resource_type).to_string();
    let mut group_paths = Vec::new();
    for path_group in build_path_groups(node, label) {
        group_paths.push(path_group);
    }
    let target_path = project_dir
        .join("target")
        .join("czar")
        .join(sanitize_segment(&task_name));
    let profiles_dir = import
        .profiles_dir
        .as_ref()
        .map(|path| resolve_import_path(workflow_root, path))
        .unwrap_or_else(|| project_dir.to_path_buf());
    let payload = serde_json::json!({
        "project_dir": project_dir.to_string_lossy().to_string(),
        "profiles_dir": profiles_dir.to_string_lossy().to_string(),
        "profile": import.profile.clone().unwrap_or_default(),
        "target": import.target.clone().unwrap_or_default(),
        "threads": import.threads.unwrap_or(1),
        "target_path": target_path.to_string_lossy().to_string(),
        "unique_id": node.unique_id,
        "selector": build_dbt_selector(node),
        "resource_type": node.resource_type,
        "node_name": node.name,
        "task_name": task_name,
    });
    let task_input = json_object_to_const_bindings(&payload);
    let task_input_type = json_value_to_schema(&payload);

    GeneratedDbtTask {
        task_name,
        group_paths,
        resource_group,
        depends_on: Vec::new(),
        template: TaskTemplateDefinition {
            executor: Some(ExecutorKind::Python),
            image: None,
            file: None,
            config_file: None,
            command: None,
            args: Vec::new(),
            wasm_mode: WasmMode::default(),
            job: None,
            module: Some("czar.dbt".to_string()),
            function: Some("run_node".to_string()),
            custom_executor: None,
            inputs: task_input,
            env: serde_json::Value::Null,
            secrets: Vec::new(),
            depends_on: Vec::new(),
            timeout: default_timeout_secs(),
            retries: default_retries(),
            retry_delay: None,
            on_failure: FailurePolicy::Skip,
            input_type: Some(task_input_type),
            output_type: None,
            validate_output: true,
            when: None,
            parallel: None,
            foreach: None,
            concurrency: None,
            parallel_mode: ParallelMode::default(),
        },
    }
}

fn build_relative_dbt_name(node: &DbtManifestNode) -> String {
    match node.resource_type.as_str() {
        "model" => relative_path_without_extension(node),
        "seed" => format!("seed.{}", relative_path_without_extension(node)),
        "snapshot" => format!("snapshot.{}", relative_path_without_extension(node)),
        "test" => format!("test.{}", sanitize_test_name(node)),
        other => format!(
            "{}.{}",
            sanitize_segment(other),
            sanitize_segment(&node.name)
        ),
    }
}

fn build_dbt_selector(node: &DbtManifestNode) -> String {
    match node.resource_type.as_str() {
        "test" => node.name.clone(),
        _ => {
            if node.fqn.len() > 1 {
                node.fqn[1..].join(".")
            } else {
                node.name.clone()
            }
        }
    }
}

fn build_path_groups(node: &DbtManifestNode, label: &str) -> Vec<String> {
    let base = relative_path_without_extension(node);
    let segments: Vec<&str> = base
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() <= 1 {
        return Vec::new();
    }
    let mut groups = Vec::new();
    let resource_prefix = match node.resource_type.as_str() {
        "model" => "path",
        "seed" => "seed_path",
        "snapshot" => "snapshot_path",
        "test" => "test_path",
        _ => "path",
    };
    for depth in 1..segments.len() {
        groups.push(format!(
            "dbt.{}.{}.{}",
            label,
            resource_prefix,
            segments[..depth].join(".")
        ));
    }
    groups
}

fn relative_path_without_extension(node: &DbtManifestNode) -> String {
    let raw = if !node.path.is_empty() {
        node.path.as_str()
    } else {
        node.original_file_path.as_str()
    };
    let path = Path::new(raw);
    let mut parts: Vec<String> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str().map(str::to_string))
        .collect();
    if matches!(
        parts.first().map(String::as_str),
        Some("models" | "seeds" | "snapshots" | "tests")
    ) {
        parts.remove(0);
    }
    if let Some(last) = parts.last_mut()
        && let Some(stem) = Path::new(last).file_stem().and_then(|value| value.to_str())
    {
        *last = stem.to_string();
    }
    parts
        .into_iter()
        .map(|segment| sanitize_segment(&segment))
        .collect::<Vec<_>>()
        .join(".")
}

fn sanitize_test_name(node: &DbtManifestNode) -> String {
    let raw = if node.path.is_empty() {
        node.name.as_str()
    } else {
        Path::new(&node.path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(node.name.as_str())
    };
    sanitize_segment(raw)
}

fn pluralize_resource_type(resource_type: &str) -> &'static str {
    match resource_type {
        "model" => "models",
        "test" => "tests",
        "seed" => "seeds",
        "snapshot" => "snapshots",
        _ => "tasks",
    }
}

pub(super) fn sanitize_segment(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len());
    let mut previous_was_separator = false;
    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() { ch } else { '_' };
        if normalized == '_' {
            if previous_was_separator {
                continue;
            }
            previous_was_separator = true;
            rendered.push('_');
        } else {
            previous_was_separator = false;
            rendered.push(normalized.to_ascii_lowercase());
        }
    }
    rendered.trim_matches('_').to_string()
}

fn json_value_to_schema(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(_) => serde_json::Value::String("str".to_string()),
        serde_json::Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                serde_json::Value::String("int".to_string())
            } else {
                serde_json::Value::String("float".to_string())
            }
        }
        serde_json::Value::Bool(_) => serde_json::Value::String("bool".to_string()),
        serde_json::Value::Array(items) => {
            let inner = items
                .first()
                .map(json_value_to_schema)
                .unwrap_or_else(|| serde_json::Value::String("str".to_string()));
            serde_json::Value::Array(vec![inner])
        }
        serde_json::Value::Object(map) => {
            let mut schema = serde_json::Map::new();
            for (key, inner) in map {
                schema.insert(key.clone(), json_value_to_schema(inner));
            }
            serde_json::Value::Object(schema)
        }
        serde_json::Value::Null => serde_json::Value::String("str".to_string()),
    }
}

fn json_object_to_const_bindings(value: &serde_json::Value) -> serde_json::Value {
    let Some(obj) = value.as_object() else {
        return serde_json::Value::Null;
    };
    let mut bindings = serde_json::Map::new();
    for (key, inner) in obj {
        bindings.insert(key.clone(), serde_json::json!({ "const": inner }));
    }
    serde_json::Value::Object(bindings)
}
