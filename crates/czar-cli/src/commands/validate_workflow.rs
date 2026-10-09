use crate::bootstrap;
use anyhow::{Context, Result};
use clap::Args;
use czar_core::error::WorkflowValidationError;
use czar_core::workflow::{ValidationOptions, WorkflowDefinition};
use czar_core::workflow_source::load_workflow_source;
use std::path::Path;

#[derive(Args, Default)]
pub struct ValidateWorkflow {
    /// Path to workflow YAML file
    pub file: String,
}

impl ValidateWorkflow {
    pub async fn execute(self) -> Result<()> {
        let workflow_path = Path::new(&self.file);
        let data = load_workflow_source(workflow_path)
            .with_context(|| format!("Failed to load workflow source: {}", self.file))?;
        let workflow_root = workflow_path.parent().unwrap_or_else(|| Path::new("."));
        let validation_options =
            if bootstrap::project_contract_validation_enabled_from(workflow_root)? {
                ValidationOptions::STRICT
            } else {
                ValidationOptions::RELAXED
            };

        let workflow = validate_workflow_yaml_str(workflow_path, &data, validation_options)
            .with_context(|| format!("workflow validation failed: {}", self.file))?;

        println!(
            "✓ Workflow '{}' is valid (schema, {} task(s))",
            workflow.name,
            workflow.tasks.len()
        );
        Ok(())
    }
}

fn validate_workflow_yaml_str(
    workflow_path: &Path,
    data: &str,
    validation_options: ValidationOptions,
) -> Result<WorkflowDefinition> {
    let workflow = WorkflowDefinition::parse_yaml_with_root(
        data,
        workflow_path.parent().unwrap_or_else(|| Path::new(".")),
    )
    .map_err(map_validation_error)
    .context("Failed to parse workflow YAML")?;
    workflow
        .validate_catalog_with_options(validation_options)
        .map_err(map_validation_error)
        .context("Workflow validation failed")?;
    compile_workflow_definition(workflow_path, &workflow, validation_options)?;
    Ok(workflow)
}

fn compile_workflow_definition(
    workflow_path: &Path,
    workflow: &WorkflowDefinition,
    validation_options: ValidationOptions,
) -> Result<()> {
    let workflow_root = workflow_path.parent().unwrap_or_else(|| Path::new("."));

    let default_flow = workflow
        .resolve_flow_with_options(None, validation_options)
        .map_err(map_validation_error)
        .context("Failed to resolve default workflow task set")?;
    default_flow
        .compile_with_options(workflow_root, &Default::default(), validation_options)
        .context("Failed to compile default workflow task set")?;

    for flow_name in workflow.flow_names() {
        let flow = workflow
            .resolve_flow_with_options(Some(&flow_name), validation_options)
            .map_err(map_validation_error)
            .with_context(|| format!("Failed to resolve flow '{}'", flow_name))?;
        flow.compile_with_options(workflow_root, &Default::default(), validation_options)
            .with_context(|| format!("Failed to compile flow '{}'", flow_name))?;
    }

    Ok(())
}

fn map_validation_error(err: WorkflowValidationError) -> anyhow::Error {
    anyhow::anyhow!(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn validate_workflow_accepts_runtime_fields_like_vars_flows_hooks_and_wasm() {
        let workflow = validate_workflow_yaml_str(
            Path::new("workflow.yaml"),
            r#"
name: polyglot_validate
vars:
  BUILD_PROFILE: debug
tasks:
  discover:
    executor: process
    command: "echo discover"
    output_type:
      ok: bool
  wasm_task:
    executor: wasm
    file: target/${{ vars.BUILD_PROFILE }}/task.wasm
    wasm_mode: cli
    function: _start
    depends_on: [discover]
flows:
  only_wasm:
    tasks: [wasm_task]
hooks:
  notify:
    on: task.failed
    tasks: "*"
    executor: process
    command: "echo hook"
"#,
            ValidationOptions::STRICT,
        )
        .expect("workflow should validate");

        assert_eq!(workflow.name, "polyglot_validate");
        assert_eq!(
            workflow.vars.get("BUILD_PROFILE").map(String::as_str),
            Some("debug")
        );
    }

    #[test]
    fn validate_workflow_rejects_foreach_cloud_run_tasks_combo() {
        let err = validate_workflow_yaml_str(
            Path::new("workflow.yaml"),
            r#"
name: invalid_foreach_cloudrun
tasks:
  discover:
    executor: process
    command: echo hi
    output_type:
      items: [str]
  process:
    executor: cloudrun
    job: demo
    depends_on: [discover]
    foreach: discover.items
    parallel_mode: cloud_run_tasks
"#,
            ValidationOptions::STRICT,
        )
        .expect_err("workflow should be rejected");

        let rendered = format!("{err:#}");
        assert!(rendered.contains("foreach"));
        assert!(rendered.contains("cloud_run_tasks"));
    }

    #[test]
    fn validate_workflow_accepts_task_without_input_fields() {
        let workflow = validate_workflow_yaml_str(
            Path::new("workflow.yaml"),
            r#"
name: no_inputs_required
tasks:
  hello:
    executor: process
    command: echo hi
"#,
            ValidationOptions::STRICT,
        )
        .expect("workflow should validate without input_type/inputs");

        assert_eq!(workflow.tasks.len(), 1);
    }

    #[test]
    fn validate_workflow_accepts_vars_refs_in_bindings() {
        let workflow = validate_workflow_yaml_str(
            Path::new("workflow.yaml"),
            r#"
name: vars_ref
vars:
  GREETING: hello
tasks:
  hello:
    executor: process
    command: echo hi
    input_type:
      greeting: str
    inputs:
      greeting:
        ref: vars.GREETING
"#,
            ValidationOptions::STRICT,
        )
        .expect("workflow should validate");

        assert_eq!(
            workflow.tasks["hello"].inputs["greeting"]["ref"],
            "vars.GREETING"
        );
    }

    #[test]
    fn validate_workflow_rejects_python_output_type_mismatch() {
        let dir = std::env::temp_dir().join(format!("czar-validate-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"czar-validate\"\nversion = \"0.1.0\"\n",
        )
        .expect("write pyproject");
        fs::write(
            dir.join("task.py"),
            r#"
from typing import TypedDict

class FetchResult(TypedDict):
    file_path: str

def main() -> FetchResult:
    return {"file_path": "/tmp/out.json"}
"#,
        )
        .expect("write task");

        let workflow_path = dir.join("workflow.yaml");
        let err = validate_workflow_yaml_str(
            &workflow_path,
            r#"
name: mismatch
types:
  FetchResult:
    raw_response_id: int
tasks:
  fetch:
    executor: python
    file: task.py
    function: main
    output_type: types.FetchResult
"#,
            ValidationOptions::STRICT,
        )
        .expect_err("validation should fail");

        let rendered = format!("{err:#}");
        assert!(rendered.contains("output_type does not match"));
        assert!(rendered.contains("raw_response_id"));
        assert!(rendered.contains("file_path"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn validate_workflow_reports_declared_and_resolved_relative_file_paths() {
        let dir = std::env::temp_dir().join(format!("czar-validate-paths-{}", Uuid::new_v4()));
        let workflow_dir = dir.join("workflows");
        fs::create_dir_all(&workflow_dir).expect("create workflow dir");
        fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"czar-validate-paths\"\nversion = \"0.1.0\"\n",
        )
        .expect("write pyproject");

        let workflow_path = workflow_dir.join("ingest.yaml");
        let err = validate_workflow_yaml_str(
            &workflow_path,
            r#"
name: relative_paths
tasks:
  seed:
    executor: python
    file: src/xroad/tasks/seed.py
"#,
            ValidationOptions::STRICT,
        )
        .expect_err("validation should fail");

        let rendered = format!("{err:#}");
        assert!(rendered.contains("src/xroad/tasks/seed.py"));
        assert!(rendered.contains(&workflow_dir.display().to_string()));
        assert!(rendered.contains("resolved relative to workflow directory"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn validate_workflow_accepts_python_output_type_mismatch_when_task_validation_disabled() {
        let dir = std::env::temp_dir().join(format!("czar-validate-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"czar-validate\"\nversion = \"0.1.0\"\n",
        )
        .expect("write pyproject");
        fs::write(
            dir.join("task.py"),
            r#"
from typing import TypedDict

class FetchResult(TypedDict):
    file_path: str

def main() -> FetchResult:
    return {"file_path": "/tmp/out.json"}
"#,
        )
        .expect("write task");

        let workflow_path = dir.join("workflow.yaml");
        let workflow = validate_workflow_yaml_str(
            &workflow_path,
            r#"
name: mismatch_disabled
types:
  FetchResult:
    raw_response_id: int
tasks:
  fetch:
    executor: python
    file: task.py
    function: main
    output_type: types.FetchResult
    validate_output: false
"#,
            ValidationOptions::STRICT,
        )
        .expect("validation should pass when task output validation is disabled");

        assert!(!workflow.tasks["fetch"].validate_output);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn validate_workflow_accepts_python_output_type_mismatch_when_contract_validation_relaxed() {
        let dir = std::env::temp_dir().join(format!("czar-validate-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"czar-validate\"\nversion = \"0.1.0\"\n",
        )
        .expect("write pyproject");
        fs::write(
            dir.join("task.py"),
            r#"
def main():
    return {"file_path": "/tmp/out.json"}
"#,
        )
        .expect("write task");

        let workflow_path = dir.join("workflow.yaml");
        validate_workflow_yaml_str(
            &workflow_path,
            r#"
name: relaxed_validation
types:
  FetchResult:
    raw_response_id: int
tasks:
  fetch:
    executor: python
    file: task.py
    function: main
    output_type: types.FetchResult
"#,
            ValidationOptions::RELAXED,
        )
        .expect("validation should pass when contract validation is globally relaxed");

        let _ = fs::remove_dir_all(dir);
    }
}
