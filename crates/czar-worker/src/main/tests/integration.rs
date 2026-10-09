/// End-to-end HTTP integration tests for czar-worker.
///
/// These tests start a real TCP server (not just calling handlers in-process)
/// and drive it through WorkerClient — the same client the orchestrator uses
/// in production. This validates the full HTTP layer: routing, JSON
/// serialization, executor dispatch, and response shape.
use std::fs;

use czar_core::models::ExecutorType;
use czar_core::worker_client::{ExecuteRequest, WorkerClient};
use tokio::sync::oneshot;
use tokio::time::{Duration, sleep};
use uuid::Uuid;

use crate::{WorkerState, run_with_shutdown};

struct Worker {
    client: WorkerClient,
    base_url: String,
    shutdown_tx: oneshot::Sender<()>,
}

/// Start a worker bound to a random OS-assigned port.  The port is only used
/// within the test process, so there is no fixed address to configure — the
/// WorkerClient is pre-constructed with the correct URL and returned alongside
/// a shutdown handle.
async fn start_worker(workflow_path: String, working_dir: String) -> Worker {
    start_worker_with_token(workflow_path, working_dir, None).await
}

async fn start_worker_with_token(
    workflow_path: String,
    working_dir: String,
    auth_token: Option<String>,
) -> Worker {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let base_url = format!("http://{addr}");
    let client = WorkerClient::new(base_url.clone()).with_token(auth_token.clone());

    tokio::spawn(async move {
        run_with_shutdown(
            listener,
            WorkerState {
                workflow_path,
                working_dir,
            },
            auth_token,
            async move {
                let _ = shutdown_rx.await;
            },
        )
        .await
        .expect("worker exited with error");
    });

    loop {
        if client.health_check().await.is_ok() {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }

    Worker {
        client,
        base_url,
        shutdown_tx,
    }
}

fn write_workflow(dir: &std::path::Path, yaml: &str) -> std::path::PathBuf {
    let path = dir.join("workflow.yaml");
    fs::write(&path, yaml).expect("write workflow");
    path
}

#[tokio::test]
async fn test_compile_and_execute_single_process_task() {
    let dir = std::env::temp_dir().join(format!("czar-worker-it-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    let workflow_path = write_workflow(
        &dir,
        r#"
name: it_single
tasks:
  greet:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"msg\":\"hello\"}'"
    output_type:
      msg: str
"#,
    );

    let worker = start_worker(
        workflow_path.to_string_lossy().into(),
        dir.to_string_lossy().into(),
    )
    .await;

    // compile
    let compiled = worker.client.compile(None).await.expect("compile failed");
    assert_eq!(compiled.tasks.len(), 1);
    assert_eq!(compiled.tasks[0].name, "greet");
    assert_eq!(compiled.tasks[0].executor, "process");
    assert!(compiled.tasks[0].depends_on.is_empty());

    // execute
    let resp = worker
        .client
        .execute(ExecuteRequest {
            task_id: Uuid::new_v4(),
            task_name: "greet".into(),
            executor_type: ExecutorType::Process,
            params: Some(serde_json::json!({
                "command": "printf 'CZAR_OUTPUT:{\"msg\":\"hello\"}'"
            })),
        })
        .await
        .expect("execute failed");

    match resp {
        czar_core::worker_client::ExecuteResponse::Success { output, .. } => {
            assert_eq!(output["msg"], "hello");
        }
        czar_core::worker_client::ExecuteResponse::Failed { error } => {
            panic!("task failed: {error}");
        }
    }

    let _ = worker.shutdown_tx.send(());
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_auth_token_required_when_configured() {
    let dir = std::env::temp_dir().join(format!("czar-worker-it-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let workflow_path = write_workflow(
        &dir,
        r#"
name: it_auth
tasks:
  greet:
    executor: process
    command: "echo hi"
"#,
    );

    let worker = start_worker_with_token(
        workflow_path.to_string_lossy().into(),
        dir.to_string_lossy().into(),
        Some("secret".to_string()),
    )
    .await;

    worker
        .client
        .compile(None)
        .await
        .expect("compile with token");

    let anonymous = WorkerClient::new(worker.base_url.clone());
    anonymous
        .health_check()
        .await
        .expect("health needs no token");
    let compile_err = anonymous.compile(None).await.expect_err("compile rejected");
    assert!(compile_err.to_string().contains("401"), "{compile_err}");
    let execute_err = anonymous
        .execute(ExecuteRequest {
            task_id: Uuid::new_v4(),
            task_name: "greet".into(),
            executor_type: ExecutorType::Process,
            params: Some(serde_json::json!({ "command": "echo hi" })),
        })
        .await
        .expect_err("execute rejected");
    assert!(execute_err.to_string().contains("401"), "{execute_err}");

    let wrong_token =
        WorkerClient::new(worker.base_url.clone()).with_token(Some("guess".to_string()));
    let wrong_err = wrong_token.compile(None).await.expect_err("wrong token");
    assert!(wrong_err.to_string().contains("401"), "{wrong_err}");

    let _ = worker.shutdown_tx.send(());
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_compile_multi_task_dependency_graph() {
    let dir = std::env::temp_dir().join(format!("czar-worker-it-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    let workflow_path = write_workflow(
        &dir,
        r#"
name: it_multi
tasks:
  step_a:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"val\":1}'"
    output_type:
      val: int
  step_b:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"val\":2}'"
    depends_on: [step_a]
    output_type:
      val: int
  step_c:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"val\":3}'"
    depends_on: [step_a]
    output_type:
      val: int
  step_d:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"val\":4}'"
    depends_on: [step_b, step_c]
    output_type:
      val: int
"#,
    );

    let worker = start_worker(
        workflow_path.to_string_lossy().into(),
        dir.to_string_lossy().into(),
    )
    .await;

    let compiled = worker.client.compile(None).await.expect("compile failed");
    assert_eq!(compiled.tasks.len(), 4);

    let by_name: std::collections::HashMap<_, _> = compiled
        .tasks
        .iter()
        .map(|t| (t.name.as_str(), t))
        .collect();

    assert!(by_name["step_a"].depends_on.is_empty());
    assert_eq!(by_name["step_b"].depends_on, ["step_a"]);
    assert_eq!(by_name["step_c"].depends_on, ["step_a"]);
    assert!(by_name["step_d"].depends_on.contains(&"step_b".to_string()));
    assert!(by_name["step_d"].depends_on.contains(&"step_c".to_string()));

    let _ = worker.shutdown_tx.send(());
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_execute_all_tasks_in_multi_task_workflow() {
    let dir = std::env::temp_dir().join(format!("czar-worker-it-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    let workflow_path = write_workflow(
        &dir,
        r#"
name: it_exec_multi
tasks:
  fetch:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"data\":42}'"
    output_type:
      data: int
  transform:
    executor: process
    command: "printf 'CZAR_OUTPUT:{\"result\":84}'"
    depends_on: [fetch]
    output_type:
      result: int
"#,
    );

    let worker = start_worker(
        workflow_path.to_string_lossy().into(),
        dir.to_string_lossy().into(),
    )
    .await;

    let fetch_resp = worker
        .client
        .execute(ExecuteRequest {
            task_id: Uuid::new_v4(),
            task_name: "fetch".into(),
            executor_type: ExecutorType::Process,
            params: Some(serde_json::json!({
                "command": "printf 'CZAR_OUTPUT:{\"data\":42}'"
            })),
        })
        .await
        .expect("fetch execute failed");

    let fetch_output = match fetch_resp {
        czar_core::worker_client::ExecuteResponse::Success { output, .. } => output,
        czar_core::worker_client::ExecuteResponse::Failed { error } => {
            panic!("fetch failed: {error}")
        }
    };
    assert_eq!(fetch_output["data"], 42);

    let transform_resp = worker
        .client
        .execute(ExecuteRequest {
            task_id: Uuid::new_v4(),
            task_name: "transform".into(),
            executor_type: ExecutorType::Process,
            params: Some(serde_json::json!({
                "command": "printf 'CZAR_OUTPUT:{\"result\":84}'"
            })),
        })
        .await
        .expect("transform execute failed");

    let transform_output = match transform_resp {
        czar_core::worker_client::ExecuteResponse::Success { output, .. } => output,
        czar_core::worker_client::ExecuteResponse::Failed { error } => {
            panic!("transform failed: {error}")
        }
    };
    assert_eq!(transform_output["result"], 84);

    let _ = worker.shutdown_tx.send(());
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_execute_unknown_executor_returns_failed_not_error() {
    let dir = std::env::temp_dir().join(format!("czar-worker-it-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let workflow_path = write_workflow(
        &dir,
        "name: it_unknown\ntasks:\n  t:\n    executor: process\n    command: echo\n",
    );

    let worker = start_worker(
        workflow_path.to_string_lossy().into(),
        dir.to_string_lossy().into(),
    )
    .await;

    // WorkerClient sends executor_type as a string; send a raw request with unknown type
    let raw: serde_json::Value = reqwest::Client::new()
        .post(format!("{}/execute", worker.base_url))
        .json(&serde_json::json!({
            "task_id": Uuid::new_v4(),
            "task_name": "t",
            "executor_type": "nonexistent_executor",
        }))
        .send()
        .await
        .expect("request failed")
        .json()
        .await
        .expect("json failed");

    assert_eq!(raw["status"], "failed");
    assert!(raw["error"].as_str().unwrap_or("").contains("Unsupported"));

    let _ = worker.shutdown_tx.send(());
    let _ = fs::remove_dir_all(&dir);
}
