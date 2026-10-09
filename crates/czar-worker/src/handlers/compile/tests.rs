use super::*;
use axum::body::to_bytes;
use axum::response::IntoResponse;
use uuid::Uuid;

#[tokio::test]
async fn test_compile_handler_valid_workflow_succeeds() {
    let dir = std::env::temp_dir().join(format!("czar-worker-compile-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"czar-worker-compile-test\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
    let workflow_path = dir.join("workflow.yaml");
    std::fs::write(
        &workflow_path,
        r#"
name: test
tasks:
  t1:
    executor: process
    command: "echo hi"
    output_type:
      data: str
  t2:
    executor: python
    module: "tasks"
    depends_on: ["t1"]
    input_type:
      t1:
        data: str
    inputs:
      t1:
        ref: tasks.t1.output
"#,
    )
    .expect("write workflow yaml");

    let state = std::sync::Arc::new(crate::WorkerState {
        workflow_path: workflow_path.to_string_lossy().to_string(),
        working_dir: ".".to_string(),
    });
    let response = compile_handler(State(state))
        .await
        .expect("compile handler should succeed")
        .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let value: serde_json::Value =
        serde_json::from_slice(&body).expect("response should be valid json");

    // Verify task definitions are correct
    assert_eq!(value["tasks"].as_array().unwrap().len(), 2);
    let t1 = &value["tasks"][0];
    let t2 = &value["tasks"][1];

    assert_eq!(t1["name"], "t1");
    assert_eq!(t1["executor"], "process");
    assert_eq!(t1["depends_on"].as_array().unwrap().len(), 0);

    assert_eq!(t2["name"], "t2");
    assert_eq!(t2["executor"], "python");
    assert_eq!(t2["depends_on"], serde_json::json!(["t1"]));

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_compile_handler_python_generator_source_succeeds() {
    let dir =
        std::env::temp_dir().join(format!("czar-worker-compile-generator-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"czar-worker-compile-test\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
    let generator_path = dir.join("generate.py");
    let workflow_path = dir.join("workflow.yaml");
    std::fs::write(
        &generator_path,
        r#"
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", required=True)
args = parser.parse_args()
Path(args.output).write_text(
    """name: test
tasks:
  t1:
    executor: process
    command: "echo hi"
""",
    encoding="utf-8",
)
"#,
    )
    .expect("write workflow generator");
    std::fs::write(
        &workflow_path,
        r#"
generate:
  kind: python
  script: generate.py
"#,
    )
    .expect("write workflow descriptor");

    let state = std::sync::Arc::new(crate::WorkerState {
        workflow_path: workflow_path.to_string_lossy().to_string(),
        working_dir: ".".to_string(),
    });
    let response = compile_handler(State(state))
        .await
        .expect("compile handler should succeed with generator")
        .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let value: serde_json::Value =
        serde_json::from_slice(&body).expect("response should be valid json");
    assert_eq!(value["tasks"].as_array().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_compile_handler_missing_file_returns_error() {
    let missing_path = format!("/tmp/does-not-exist-{}.yaml", Uuid::new_v4());
    let state = std::sync::Arc::new(crate::WorkerState {
        workflow_path: missing_path.clone(),
        working_dir: ".".to_string(),
    });
    let result = compile_handler(State(state)).await;
    match result {
        Ok(_) => panic!("missing workflow should return error"),
        Err(error) => assert_eq!(error.0, StatusCode::BAD_REQUEST),
    }
}

#[tokio::test]
async fn test_compile_handler_invalid_yaml_returns_bad_request() {
    let dir = std::env::temp_dir().join(format!("czar-worker-compile-invalid-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let workflow_path = dir.join("workflow.yaml");
    std::fs::write(&workflow_path, "name: [unterminated").expect("write invalid workflow yaml");

    let state = std::sync::Arc::new(crate::WorkerState {
        workflow_path: workflow_path.to_string_lossy().to_string(),
        working_dir: ".".to_string(),
    });
    let result = compile_handler(State(state)).await;

    let _ = std::fs::remove_dir_all(dir);
    match result {
        Ok(_) => panic!("invalid yaml should return error"),
        Err(error) => assert_eq!(error.0, StatusCode::BAD_REQUEST),
    }
}

#[tokio::test]
async fn test_compile_handler_missing_tasks_returns_bad_request() {
    let dir = std::env::temp_dir().join(format!(
        "czar-worker-compile-missing-tasks-{}",
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let workflow_path = dir.join("workflow.yaml");
    std::fs::write(&workflow_path, r#"name: test"#).expect("write workflow yaml without tasks");

    let state = std::sync::Arc::new(crate::WorkerState {
        workflow_path: workflow_path.to_string_lossy().to_string(),
        working_dir: ".".to_string(),
    });
    let result = compile_handler(State(state)).await;

    let _ = std::fs::remove_dir_all(dir);
    match result {
        Ok(_) => panic!("workflow without tasks should return error"),
        Err(error) => {
            assert_eq!(error.0, StatusCode::BAD_REQUEST);
            assert!(!error.1.is_empty());
        }
    }
}

#[tokio::test]
async fn test_compile_handler_rejects_invalid_dependency() {
    let dir = std::env::temp_dir().join(format!("czar-worker-compile-bad-dep-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let workflow_path = dir.join("workflow.yaml");
    std::fs::write(
        &workflow_path,
        r#"
name: test
tasks:
  task1:
    executor: process
    command: "echo hi"
    depends_on: ["nonexistent"]
"#,
    )
    .expect("write workflow yaml with bad dependency");

    let state = std::sync::Arc::new(crate::WorkerState {
        workflow_path: workflow_path.to_string_lossy().to_string(),
        working_dir: ".".to_string(),
    });
    let result = compile_handler(State(state)).await;

    let _ = std::fs::remove_dir_all(dir);
    match result {
        Ok(_) => panic!("workflow with invalid dependency should return error"),
        Err(error) => {
            assert_eq!(error.0, StatusCode::BAD_REQUEST);
            assert!(!error.1.is_empty());
        }
    }
}

#[tokio::test]
async fn test_compile_handler_rejects_missing_required_field() {
    let dir = std::env::temp_dir().join(format!(
        "czar-worker-compile-missing-field-{}",
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let workflow_path = dir.join("workflow.yaml");
    std::fs::write(
        &workflow_path,
        r#"
name: test
tasks:
  task1:
    executor: library
"#,
    )
    .expect("write workflow yaml with missing required field");

    let state = std::sync::Arc::new(crate::WorkerState {
        workflow_path: workflow_path.to_string_lossy().to_string(),
        working_dir: ".".to_string(),
    });
    let result = compile_handler(State(state)).await;

    let _ = std::fs::remove_dir_all(dir);
    match result {
        Ok(_) => panic!("workflow with missing required field should return error"),
        Err(error) => {
            assert_eq!(error.0, StatusCode::BAD_REQUEST);
            assert!(!error.1.is_empty());
        }
    }
}

#[tokio::test]
async fn test_compile_handler_rejects_cycle() {
    let dir = std::env::temp_dir().join(format!("czar-worker-compile-cycle-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let workflow_path = dir.join("workflow.yaml");
    std::fs::write(
        &workflow_path,
        r#"
name: test
tasks:
  a:
    executor: process
    command: "echo a"
    depends_on: ["b"]
  b:
    executor: process
    command: "echo b"
    depends_on: ["a"]
"#,
    )
    .expect("write workflow yaml with cycle");

    let state = std::sync::Arc::new(crate::WorkerState {
        workflow_path: workflow_path.to_string_lossy().to_string(),
        working_dir: ".".to_string(),
    });
    let result = compile_handler(State(state)).await;

    let _ = std::fs::remove_dir_all(dir);
    match result {
        Ok(_) => panic!("workflow with cycle should return error"),
        Err(error) => {
            assert_eq!(error.0, StatusCode::BAD_REQUEST);
            assert!(
                error.1.contains("Failed to compile workflow")
                    || error.1.contains("cycle")
                    || error.1.contains("Cycle")
            );
        }
    }
}
