use super::*;
use crate::compiled::python_contracts::introspect_python_signature_with_script;

#[test]
fn test_compile_python_module_without_file_skips_signature_introspection() {
    let dir = TempDir::new().expect("temp dir");
    write_pyproject(&dir);
    let yaml = r#"
name: python-module-only
tasks:
  only:
    executor: python
    module: app.tasks
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).expect("workflow yaml");
    let compiled = workflow.compile(dir.path()).expect("compile");
    let idx = *compiled.name_index.get("only").expect("task index");
    let task = &compiled.tasks[idx];

    assert!(task.file.is_none());
    assert_eq!(task.python_path.as_deref(), Some(dir.path()));
    assert!(task.signature.is_none());
}

#[test]
fn test_compile_python_module_requires_pyproject_in_workflow_root() {
    let dir = TempDir::new().expect("temp dir");
    let yaml = r#"
name: python-module-only
tasks:
  only:
    executor: python
    module: app.tasks
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).expect("workflow yaml");
    let err = workflow
        .compile(dir.path())
        .expect_err("missing pyproject should fail compilation");

    assert!(
        err.to_string()
            .contains("requires a pyproject.toml in workflow root"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_get_inspect_script_is_cached_on_second_call() {
    let _guard = script_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let first = get_inspect_script().expect("first inspect script path");
    assert!(first.exists(), "inspect script should be written");

    let second = get_inspect_script().expect("second inspect script path");
    assert_eq!(first, second, "inspect script path should be stable");
}

#[test]
fn test_get_inspect_script_recreates_missing_script_file() {
    let _guard = script_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = get_inspect_script().expect("inspect script path");
    if path.exists() {
        std::fs::remove_file(&path).expect("remove inspect script");
    }
    assert!(
        !path.exists(),
        "inspect script should be removed for this test"
    );

    let recreated = get_inspect_script().expect("recreate inspect script");
    assert_eq!(recreated, path);
    assert!(recreated.exists(), "inspect script should be recreated");
}

#[test]
fn test_get_inspect_script_overwrites_stale_cached_content() {
    let _guard = script_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = get_inspect_script().expect("inspect script path");
    let original = std::fs::read_to_string(&path).expect("read inspect script");
    let _restore = ScriptRestore {
        path: path.clone(),
        original,
    };

    std::fs::write(&path, "# stale inspect script\n").expect("overwrite inspect script");

    let refreshed = get_inspect_script().expect("refresh inspect script");
    assert_eq!(refreshed, path);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read refreshed inspect script"),
        include_str!("../runtime/inspect_task.py")
    );
}

#[test]
fn test_introspect_python_signature_surfaces_python_side_error() {
    let _guard = script_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = TempDir::new().expect("temp dir");
    write_pyproject(&dir);
    let file = write_file(
        &dir,
        "task.py",
        r#"
def actual_task(x: int) -> int:
    return x + 1
"#,
    );

    let err = introspect_python_signature(&file, "missing_task", dir.path())
        .expect_err("missing function should fail introspection");
    assert!(
        err.contains("Task analysis error") || err.contains("Failed to parse introspection output"),
        "expected introspection failure, got: {err}"
    );
}

#[test]
fn test_introspect_python_signature_reports_non_json_success_output() {
    let _guard = script_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = TempDir::new().expect("temp dir");
    write_pyproject(&dir);
    let script = write_file(
        &dir,
        "inspect_task.py",
        r#"
import sys
print("not-json")
sys.exit(0)
"#,
    );
    let file = write_file(
        &dir,
        "task.py",
        r#"
def main(x: int) -> int:
    return x + 1
"#,
    );

    let err = introspect_python_signature_with_script(&file, "main", dir.path(), &script)
        .expect_err("non-json stdout should fail parsing");
    assert!(err.contains("Failed to parse introspection output"));
}

#[test]
fn test_introspect_python_signature_success_path() {
    let _guard = script_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = TempDir::new().expect("temp dir");
    write_pyproject(&dir);
    let file = write_file(
        &dir,
        "task.py",
        r#"
def main(x: int, y: str) -> bool:
    return True
"#,
    );

    let value =
        introspect_python_signature(&file, "main", dir.path()).expect("successful introspection");
    assert_eq!(value["inputs"]["x"], "int");
    assert_eq!(value["inputs"]["y"], "str");
    assert_eq!(value["output"], "bool");
}

#[test]
fn test_introspect_python_signature_ignores_context_parameter() {
    let _guard = script_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = TempDir::new().expect("temp dir");
    let sdk_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../python-sdk")
        .canonicalize()
        .expect("canonicalize python-sdk path");
    std::fs::write(
        dir.path().join("pyproject.toml"),
        format!(
            r#"[project]
name = "context-introspection"
version = "0.1.0"
requires-python = ">=3.12"
dependencies = ["czar"]

[tool.uv.sources]
czar = {{ path = "{}" }}
"#,
            sdk_path.display()
        ),
    )
    .expect("write pyproject");
    let file = write_file(
        &dir,
        "task.py",
        r#"
from czar import Context

def main(x: int, ctx: Context) -> bool:
    return ctx.parallel_index is None and x > 0
"#,
    );

    let value =
        introspect_python_signature(&file, "main", dir.path()).expect("successful introspection");
    assert_eq!(value["inputs"]["x"], "int");
    assert_eq!(value["inputs"].as_object().map(|m| m.len()), Some(1));
    assert_eq!(value["output"], "bool");
}

#[test]
fn test_introspect_python_signature_reports_script_runtime_failure() {
    let _guard = script_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = TempDir::new().expect("temp dir");
    write_pyproject(&dir);
    let script = write_file(
        &dir,
        "inspect_task.py",
        r#"
raise RuntimeError("broken inspect script")
"#,
    );
    let file = write_file(
        &dir,
        "task.py",
        r#"
def main(x: int) -> int:
    return x + 1
"#,
    );

    let err = introspect_python_signature_with_script(&file, "main", dir.path(), &script)
        .expect_err("runtime failure should return stderr-based error");
    assert!(err.contains("Introspection failed"));
}

#[test]
fn test_introspect_python_signature_requires_pyproject_root() {
    let _guard = script_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = TempDir::new().expect("temp dir");
    let file = write_file(
        &dir,
        "task.py",
        r#"
def main(x: int) -> int:
    return x + 1
"#,
    );

    let err = introspect_python_signature(&file, "main", dir.path())
        .expect_err("missing pyproject should fail introspection");
    assert!(err.contains("pyproject.toml"), "unexpected error: {err}");
}

#[test]
fn test_compile_python_file_uses_task_file_pyproject_ancestor() {
    let _guard = script_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let workflow_root = TempDir::new().expect("workflow root");
    let python_project = workflow_root.path().join("python-project");
    std::fs::create_dir_all(&python_project).expect("create python project");
    std::fs::write(
        python_project.join("pyproject.toml"),
        "[project]\nname = \"nested\"\nversion = \"0.1.0\"\n",
    )
    .expect("write nested pyproject");
    let task_file = python_project.join("task.py");
    std::fs::write(
        &task_file,
        r#"
def run(x: int) -> int:
    return x
"#,
    )
    .expect("write task file");

    let yaml = r#"
name: nested-python-project
tasks:
  only:
    executor: python
    file: python-project/task.py
    function: run
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).expect("workflow yaml");
    let compiled = workflow.compile(workflow_root.path()).expect("compile");
    let idx = *compiled.name_index.get("only").expect("task index");
    let task = &compiled.tasks[idx];

    assert_eq!(task.file.as_deref(), Some(task_file.as_path()));
    assert_eq!(task.python_path.as_deref(), Some(python_project.as_path()));
}
