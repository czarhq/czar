//! Tests for Python task type extraction during workflow compilation.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use czar_core::workflow::Workflow;

/// Creates a temporary Python file with the given content and returns its path.
fn create_temp_python_file(dir: &TempDir, filename: &str, content: &str) -> PathBuf {
    let path = dir.path().join(filename);
    fs::write(&path, content).expect("Failed to write temp file");
    path
}

fn write_pyproject(dir: &TempDir) {
    fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"czar-type-test\"\nversion = \"0.1.0\"\n",
    )
    .expect("Failed to write pyproject.toml");
}

/// Creates a workflow YAML string for testing.
fn create_workflow_yaml(task_file: &str, function: &str) -> String {
    format!(
        r#"
name: test_workflow
tasks:
  test_task:
    executor: python
    file: {task_file}
    function: {function}
    input_type:
      a: int
      b: int
    inputs:
      a:
        const: 1
      b:
        const: 2
    output_type: int
"#,
        task_file = task_file,
        function = function
    )
}

#[test]
fn test_typed_python_task_compiles_with_signature() {
    let temp_dir = TempDir::new().unwrap();
    write_pyproject(&temp_dir);

    // Create a properly typed Python function
    let py_content = r#"
def add(a: int, b: int) -> int:
    return a + b
"#;
    let py_path = create_temp_python_file(&temp_dir, "typed_task.py", py_content);

    let yaml = create_workflow_yaml(py_path.to_str().unwrap(), "add");
    let workflow: Workflow = serde_yaml::from_str(&yaml).unwrap();

    // This should succeed since all arguments are typed
    let result = workflow.compile(temp_dir.path());

    assert!(result.is_ok(), "Compilation should succeed for typed tasks");

    let compiled = result.unwrap();
    assert_eq!(compiled.tasks.len(), 1);

    let task = &compiled.tasks[0];
    assert!(task.signature.is_some(), "Signature should be extracted");

    let sig = task.signature.as_ref().unwrap();
    assert!(sig.get("inputs").is_some(), "Signature should have inputs");
    assert!(sig.get("output").is_some(), "Signature should have output");

    let inputs = sig.get("inputs").unwrap().as_object().unwrap();
    assert_eq!(inputs.get("a").unwrap().as_str().unwrap(), "int");
    assert_eq!(inputs.get("b").unwrap().as_str().unwrap(), "int");
    assert_eq!(sig.get("output").unwrap().as_str().unwrap(), "int");
}

#[test]
fn test_typed_dict_python_output_matches_declared_alias_shape() {
    let temp_dir = TempDir::new().unwrap();
    write_pyproject(&temp_dir);

    let py_content = r#"
from typing import TypedDict

class FetchResult(TypedDict):
    file_path: str
    item_ids: list[str]

def fetch() -> FetchResult:
    return {"file_path": "/tmp/out.json", "item_ids": ["a", "b"]}
"#;
    let py_path = create_temp_python_file(&temp_dir, "typed_dict_task.py", py_content);

    let yaml = format!(
        r#"
name: typed_dict_output
types:
  FetchResult:
    file_path: str
    item_ids: list[str]
tasks:
  fetch:
    executor: python
    file: {}
    function: fetch
    output_type: types.FetchResult
"#,
        py_path.to_string_lossy()
    );
    let workflow: Workflow = serde_yaml::from_str(&yaml).unwrap();

    let compiled = workflow.compile(temp_dir.path()).expect("compile");
    let sig = compiled.tasks[0].signature.as_ref().expect("signature");
    assert_eq!(
        sig.get("output").cloned().unwrap(),
        serde_json::json!({"file_path": "str", "item_ids": ["str"]})
    );
}

#[test]
fn test_python_output_type_mismatch_fails_compilation() {
    let temp_dir = TempDir::new().unwrap();
    write_pyproject(&temp_dir);

    let py_content = r#"
from typing import TypedDict

class FetchResult(TypedDict):
    file_path: str

def fetch() -> FetchResult:
    return {"file_path": "/tmp/out.json"}
"#;
    let py_path = create_temp_python_file(&temp_dir, "mismatch_task.py", py_content);

    let yaml = format!(
        r#"
name: typed_dict_mismatch
types:
  FetchResult:
    raw_response_id: int
tasks:
  fetch:
    executor: python
    file: {}
    function: fetch
    output_type: types.FetchResult
"#,
        py_path.to_string_lossy()
    );
    let workflow: Workflow = serde_yaml::from_str(&yaml).unwrap();

    let err = workflow
        .compile(temp_dir.path())
        .expect_err("mismatch should fail");
    let rendered = err.to_string();
    assert!(rendered.contains("output_type does not match"));
    assert!(rendered.contains("raw_response_id"));
    assert!(rendered.contains("file_path"));
}

#[test]
fn test_untyped_python_task_fails_compilation() {
    let temp_dir = TempDir::new().unwrap();
    write_pyproject(&temp_dir);

    // Create a Python function with untyped arguments
    let py_content = r#"
def untyped_add(a, b):
    return a + b
"#;
    let py_path = create_temp_python_file(&temp_dir, "untyped_task.py", py_content);

    let yaml = create_workflow_yaml(py_path.to_str().unwrap(), "untyped_add");
    let workflow: Workflow = serde_yaml::from_str(&yaml).unwrap();

    // This should fail because arguments are not typed
    let result = workflow.compile(temp_dir.path());

    assert!(result.is_err(), "Compilation should fail for untyped tasks");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("untyped") || err_msg.contains("Introspection"),
        "Error message should mention untyped arguments or introspection failure: {}",
        err_msg
    );
}

#[test]
fn test_partially_typed_python_task_fails_compilation() {
    let temp_dir = TempDir::new().unwrap();
    write_pyproject(&temp_dir);

    // Create a Python function with only some typed arguments
    let py_content = r#"
def partial_add(a: int, b) -> int:
    return a + b
"#;
    let py_path = create_temp_python_file(&temp_dir, "partial_task.py", py_content);

    let yaml = create_workflow_yaml(py_path.to_str().unwrap(), "partial_add");
    let workflow: Workflow = serde_yaml::from_str(&yaml).unwrap();

    // This should fail because 'b' is not typed
    let result = workflow.compile(temp_dir.path());

    assert!(
        result.is_err(),
        "Compilation should fail for partially typed tasks"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("'b'") || err_msg.contains("untyped"),
        "Error message should mention the untyped argument 'b': {}",
        err_msg
    );
}
