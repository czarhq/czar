use super::*;

#[tokio::test]
async fn test_execute_handler_python_task_file_resolves_python_path_from_task_file() {
    let base = std::env::temp_dir().join(format!("czar-worker-python-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");
    fs::write(
        base.join("pyproject.toml"),
        "[project]\nname = \"czar-worker-python\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
    fs::write(base.join("workflow.yaml"), "name: wf\ntasks: {}\n").expect("write workflow");
    fs::write(
        base.join("task.py"),
        "def run():\n    return {'ok': True}\n",
    )
    .expect("write task");
    fs::write(
        base.join("runner.py"),
        r#"
import json
import os

print("CZAR_OUTPUT:" + json.dumps({
    "python_path": os.environ.get("PYTHONPATH"),
    "task_file": os.environ.get("CZAR_TASK_FILE"),
}))
"#,
    )
    .expect("write runner");

    let value = run_handler_with_state(
        crate::WorkerState {
            workflow_path: base.join("workflow.yaml").to_string_lossy().to_string(),
            working_dir: base.to_string_lossy().to_string(),
        },
        ExecuteRequest {
            executor_type: ExecutorType::Python.into(),
            params: Some(serde_json::json!({
                "task_file": "task.py",
                "runner_path": base.join("runner.py"),
            })),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(value["status"], "success");
    assert!(
        value["output"]["task_file"]
            .as_str()
            .unwrap_or("")
            .ends_with("task.py")
    );
    let python_path = value["output"]["python_path"].as_str().unwrap_or("");
    assert!(
        std::env::split_paths(python_path)
            .next()
            .map(|path| path == base)
            .unwrap_or(false)
    );

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn test_execute_handler_python_module_resolves_python_path_from_workflow_root() {
    let base = std::env::temp_dir().join(format!("czar-worker-python-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");
    fs::write(
        base.join("pyproject.toml"),
        "[project]\nname = \"czar-worker-python\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
    fs::write(base.join("workflow.yaml"), "name: wf\ntasks: {}\n").expect("write workflow");
    fs::write(
        base.join("runner.py"),
        r#"
import json
import os

print("CZAR_OUTPUT:" + json.dumps({
    "python_path": os.environ.get("PYTHONPATH"),
    "task_module": os.environ.get("CZAR_TASK_MODULE"),
}))
"#,
    )
    .expect("write runner");

    let value = run_handler_with_state(
        crate::WorkerState {
            workflow_path: base.join("workflow.yaml").to_string_lossy().to_string(),
            working_dir: base.to_string_lossy().to_string(),
        },
        ExecuteRequest {
            executor_type: ExecutorType::Python.into(),
            params: Some(serde_json::json!({
                "task_module": "pkg.tasks",
                "runner_path": base.join("runner.py"),
            })),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(value["status"], "success");
    assert_eq!(value["output"]["task_module"], "pkg.tasks");
    let python_path = value["output"]["python_path"].as_str().unwrap_or("");
    assert!(
        std::env::split_paths(python_path)
            .next()
            .map(|path| path == base)
            .unwrap_or(false)
    );

    let _ = fs::remove_dir_all(&base);
}
