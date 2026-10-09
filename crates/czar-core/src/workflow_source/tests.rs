use super::load_workflow_source;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn tmp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("czar-workflow-source-{label}-{}", Uuid::new_v4()))
}

fn write_pyproject(dir: &Path) {
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"czar-workflow-source-test\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
}

#[test]
fn test_load_workflow_source_reads_yaml_file() {
    let dir = tmp_dir("yaml");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("wf.yaml");
    std::fs::write(
        &path,
        "name: wf\ntasks:\n  a:\n    executor: process\n    command: \"echo a\"\n",
    )
    .expect("write yaml");

    let loaded = load_workflow_source(&path).expect("load yaml");
    assert!(loaded.contains("name: wf"));
    assert!(loaded.contains("tasks:"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_load_workflow_source_executes_python_generator_descriptor() {
    let dir = tmp_dir("descriptor");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    write_pyproject(&dir);
    let script = dir.join("generate.py");
    std::fs::write(
        &script,
        r#"
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", required=True)
args = parser.parse_args()
Path(args.output).write_text("name: wf\ntasks:\n  a:\n    executor: process\n    command: \"echo a\"\n", encoding="utf-8")
"#,
    )
    .expect("write generator");
    let descriptor = dir.join("wf.yaml");
    std::fs::write(
        &descriptor,
        r#"
generate:
  kind: python
  script: generate.py
  output: wf.generated.yaml
"#,
    )
    .expect("write descriptor");

    let loaded = load_workflow_source(&descriptor).expect("load generated yaml");
    assert!(loaded.contains("name: wf"));
    assert!(loaded.contains("executor: process"));
    assert!(dir.join("wf.generated.yaml").exists());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_load_workflow_source_fails_when_generator_output_is_empty() {
    let dir = tmp_dir("empty");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    write_pyproject(&dir);
    let script_path = dir.join("generate.py");
    let descriptor_path = dir.join("wf.yaml");
    std::fs::write(
        &script_path,
        r#"
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", required=True)
args = parser.parse_args()
Path(args.output).write_text("", encoding="utf-8")
"#,
    )
    .expect("write generator");
    std::fs::write(
        &descriptor_path,
        r#"
generate:
  kind: python
  script: generate.py
  output: wf.generated.yaml
"#,
    )
    .expect("write descriptor");

    let error = load_workflow_source(&descriptor_path).expect_err("empty output should fail");
    assert!(error.to_string().contains("empty output"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_load_workflow_source_rejects_direct_python_source() {
    let dir = tmp_dir("py-reject");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("generate.py");
    std::fs::write(&path, "print('hello')\n").expect("write generator");

    let error = load_workflow_source(&path).expect_err("direct python source should fail");
    assert!(error.to_string().contains("must be a YAML file"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_load_workflow_source_descriptor_allows_numeric_args_without_quotes() {
    let dir = tmp_dir("descriptor-numeric-args");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    write_pyproject(&dir);
    let script = dir.join("generate.py");
    std::fs::write(
        &script,
        r#"
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", required=True)
parser.add_argument("--layers", type=int, required=True)
args = parser.parse_args()
Path(args.output).write_text(f"name: wf\ntasks:\n  t{args.layers}:\n    executor: process\n    command: \"echo ok\"\n", encoding="utf-8")
"#,
    )
    .expect("write generator");
    let descriptor = dir.join("wf.yaml");
    std::fs::write(
        &descriptor,
        r#"
generate:
  kind: python
  script: generate.py
  output: wf.generated.yaml
  args:
    - --layers
    - 7
"#,
    )
    .expect("write descriptor");

    let loaded = load_workflow_source(&descriptor).expect("load generated yaml");
    assert!(loaded.contains("t7"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_load_workflow_source_descriptor_uses_cached_output_by_default() {
    let dir = tmp_dir("descriptor-cache");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    write_pyproject(&dir);
    let script = dir.join("generate.py");
    std::fs::write(
        &script,
        r#"
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", required=True)
args = parser.parse_args()
count_path = Path("count.txt")
count = int(count_path.read_text(encoding="utf-8")) + 1 if count_path.exists() else 1
count_path.write_text(str(count), encoding="utf-8")
Path(args.output).write_text(f"name: wf{count}\ntasks:\n  a:\n    executor: process\n    command: \"echo a\"\n", encoding="utf-8")
"#,
    )
    .expect("write generator");
    let descriptor = dir.join("wf.yaml");
    std::fs::write(
        &descriptor,
        r#"
generate:
  kind: python
  script: generate.py
  output: cache.generated.yaml
"#,
    )
    .expect("write descriptor");

    let first = load_workflow_source(&descriptor).expect("first load");
    let second = load_workflow_source(&descriptor).expect("second load");
    assert!(first.contains("name: wf1"));
    assert_eq!(first, second);
    let count = std::fs::read_to_string(dir.join("count.txt")).expect("count exists");
    assert_eq!(count.trim(), "1");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_load_workflow_source_descriptor_refresh_true_reruns_generator() {
    let dir = tmp_dir("descriptor-refresh");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    write_pyproject(&dir);
    let script = dir.join("generate.py");
    std::fs::write(
        &script,
        r#"
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", required=True)
args = parser.parse_args()
count_path = Path("count.txt")
count = int(count_path.read_text(encoding="utf-8")) + 1 if count_path.exists() else 1
count_path.write_text(str(count), encoding="utf-8")
Path(args.output).write_text(f"name: wf{count}\ntasks:\n  a:\n    executor: process\n    command: \"echo a\"\n", encoding="utf-8")
"#,
    )
    .expect("write generator");
    let descriptor = dir.join("wf.yaml");
    std::fs::write(
        &descriptor,
        r#"
generate:
  kind: python
  script: generate.py
  output: refresh.generated.yaml
  refresh: true
"#,
    )
    .expect("write descriptor");

    let first = load_workflow_source(&descriptor).expect("first load");
    let second = load_workflow_source(&descriptor).expect("second load");
    assert!(first.contains("name: wf1"));
    assert!(second.contains("name: wf2"));
    let count = std::fs::read_to_string(dir.join("count.txt")).expect("count exists");
    assert_eq!(count.trim(), "2");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_load_workflow_source_python_generator_requires_pyproject() {
    let dir = tmp_dir("descriptor-missing-pyproject");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("generate.py");
    std::fs::write(
        &script,
        r#"
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", required=True)
args = parser.parse_args()
Path(args.output).write_text("name: wf\n", encoding="utf-8")
"#,
    )
    .expect("write generator");
    let descriptor = dir.join("wf.yaml");
    std::fs::write(
        &descriptor,
        r#"
generate:
  kind: python
  script: generate.py
"#,
    )
    .expect("write descriptor");

    let error =
        load_workflow_source(&descriptor).expect_err("missing pyproject should fail generator");
    assert!(
        error.to_string().contains("pyproject.toml ancestor"),
        "unexpected error: {error}"
    );

    let _ = std::fs::remove_dir_all(dir);
}
