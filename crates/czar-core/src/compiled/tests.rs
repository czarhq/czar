use super::*;
use crate::workflow::{ExecutorKind, Workflow};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

pub(super) fn write_file(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).expect("write file");
    path
}

pub(super) fn write_pyproject(dir: &TempDir) {
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"czar-test\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
}

pub(super) fn script_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Default)]
pub(super) struct ScriptRestore {
    pub(super) path: std::path::PathBuf,
    pub(super) original: String,
}

impl Drop for ScriptRestore {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.path, &self.original);
    }
}

mod build_workflow;
mod interpolation;
mod python;

#[test]
fn test_interpolate_supports_workflow_and_builtin_vars() {
    let vars = BTreeMap::from([
        ("BUILD_PROFILE".to_string(), "release".to_string()),
        ("platform.lib_ext".to_string(), "so".to_string()),
    ]);

    assert_eq!(
        interpolate(
            "target/${{ vars.BUILD_PROFILE }}/libfoo.${{ platform.lib_ext }}",
            &vars
        ),
        "target/release/libfoo.so"
    );
}

#[test]
fn test_compile_allows_missing_library_and_wasm_artifacts() {
    let dir = TempDir::new().expect("temp dir");
    let workflow_path = write_file(
        &dir,
        "workflow.yaml",
        r#"
name: artifact_build
tasks:
  library_task:
    executor: library
    file: target/debug/libgenerated.${{ platform.lib_ext }}
  wasm_task:
    executor: wasm
    file: target/wasm32-wasip1/debug/generated.wasm
"#,
    );

    let workflow = Workflow::load(&workflow_path).expect("load workflow");
    let compiled = workflow.compile(dir.path()).expect("compile workflow");

    assert_eq!(
        compiled.tasks[0].file.as_ref().unwrap(),
        &dir.path()
            .join(format!("target/debug/libgenerated.{}", PLATFORM_LIB_EXT))
    );
    assert_eq!(
        compiled.tasks[1].file.as_ref().unwrap(),
        &dir.path().join("target/wasm32-wasip1/debug/generated.wasm")
    );
}

#[test]
fn test_topo_sort_returns_cycle_error_when_graph_is_cyclic() {
    let tasks = vec![
        CompiledTask {
            name: "a".to_string(),
            executor: crate::workflow::ExecutorKind::Process,
            command: Some("echo a".to_string()),
            depends_on: vec![1],
            ..Default::default()
        },
        CompiledTask {
            name: "b".to_string(),
            executor: crate::workflow::ExecutorKind::Process,
            command: Some("echo b".to_string()),
            depends_on: vec![0],
            ..Default::default()
        },
    ];

    let err = topo_sort(&tasks).expect_err("cyclic graph should fail topo sort");
    assert!(matches!(
        err,
        crate::error::CzarError::InvalidWorkflow(
            crate::error::WorkflowValidationError::Cycle { .. }
        )
    ));
}
