use std::fs;

use czar_core::workflow::Workflow;
use tempfile::TempDir;

fn write_file(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).expect("write file");
    path
}

#[test]
fn test_compile_non_python_workflow_builds_topology_and_paths() {
    let dir = TempDir::new().expect("temp dir");
    let process_file = write_file(&dir, "task.sh", "#!/bin/sh\necho ok\n");
    let library_file = write_file(&dir, "task.so", "not-a-real-shared-object");

    let yaml = r#"
name: mixed
tasks:
  extract:
    executor: process
    file: task.sh
    output_type:
      data: str
  launch:
    executor: cloudrun
    job: cloud-job
    output_type:
      status: str
  native:
    executor: library
    file: task.so
    output_type:
      result: int
  aggregate:
    executor: process
    command: "echo done"
    depends_on: [extract, launch, native]
    input_type:
      upstream:
        extract:
          data: str
        launch:
          status: str
        native:
          result: int
    inputs:
      upstream:
        const:
          extract: {}
          launch: {}
          native: {}
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).expect("workflow yaml");
    let compiled = workflow.compile(dir.path()).expect("compile");

    assert_eq!(compiled.name, "mixed");
    assert_eq!(compiled.tasks.len(), 4);
    assert_eq!(compiled.topo.len(), 4);
    assert_eq!(compiled.root, dir.path());

    let extract_idx = *compiled.name_index.get("extract").expect("extract index");
    let launch_idx = *compiled.name_index.get("launch").expect("launch index");
    let native_idx = *compiled.name_index.get("native").expect("native index");
    let aggregate_idx = *compiled
        .name_index
        .get("aggregate")
        .expect("aggregate index");

    assert_eq!(
        compiled.tasks[extract_idx]
            .file
            .as_ref()
            .expect("process file"),
        &process_file.canonicalize().expect("canonical process")
    );
    assert!(compiled.tasks[launch_idx].file.is_none());
    assert_eq!(
        compiled.tasks[native_idx]
            .file
            .as_ref()
            .expect("library file"),
        &library_file.canonicalize().expect("canonical lib")
    );
    assert_eq!(
        compiled.tasks[aggregate_idx].depends_on,
        vec![extract_idx, launch_idx, native_idx]
    );
}

#[test]
fn test_compile_allows_missing_process_file_for_build_then_run_workflows() {
    let dir = TempDir::new().expect("temp dir");
    let yaml = r#"
name: missing_file
tasks:
  one:
    executor: process
    file: does-not-exist.sh
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).expect("workflow yaml");
    let compiled = workflow
        .compile(dir.path())
        .expect("compile should succeed");
    let idx = *compiled.name_index.get("one").expect("task index");
    let expected = dir.path().join("does-not-exist.sh");
    assert_eq!(
        compiled.tasks[idx].file.as_deref(),
        Some(expected.as_path())
    );
}

#[test]
fn test_compile_accepts_absolute_process_file_path() {
    let dir = TempDir::new().expect("temp dir");
    let abs_path = write_file(&dir, "abs_task.sh", "#!/bin/sh\necho absolute\n")
        .canonicalize()
        .expect("canonical path");

    let yaml = format!(
        r#"
name: absolute
tasks:
  one:
    executor: process
    file: {}
"#,
        abs_path.display()
    );
    let workflow: Workflow = serde_yaml::from_str(&yaml).expect("workflow yaml");
    let compiled = workflow.compile(dir.path()).expect("compile");
    let idx = *compiled.name_index.get("one").expect("task index");
    assert_eq!(
        compiled.tasks[idx].file.as_ref().expect("resolved file"),
        &abs_path
    );
}
