use super::*;
use axum::body::to_bytes;
use axum::response::IntoResponse;
use std::net::SocketAddr;
use tokio::sync::oneshot;
use tokio::time::{Duration, sleep};

#[test]
fn test_args_defaults() {
    let args = Args::parse_from(["czar-worker"]);
    assert_eq!(args.addr, "127.0.0.1:8081");
    assert_eq!(args.workflow_path, "workflow.yaml");
    assert_eq!(args.working_dir, ".");
}

#[test]
fn test_args_custom_values() {
    let args = Args::parse_from([
        "czar-worker",
        "--addr",
        "0.0.0.0:9000",
        "--workflow-path",
        "/tmp/workflow.yaml",
        "--working-dir",
        "/tmp",
    ]);
    assert_eq!(args.addr, "0.0.0.0:9000");
    assert_eq!(args.workflow_path, "/tmp/workflow.yaml");
    assert_eq!(args.working_dir, "/tmp");
}

#[test]
fn test_args_auth_token() {
    let args = Args::parse_from(["czar-worker", "--auth-token", "secret"]);
    assert_eq!(args.auth_token.as_deref(), Some("secret"));
}

#[tokio::test]
async fn test_health_handler_payload() {
    let response = health_handler().await.into_response();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid json");
    assert_eq!(json["status"], "healthy");
    assert_eq!(json["service"], "czar-worker");
}

#[tokio::test]
async fn test_run_with_shutdown_serves_health_route() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let state = WorkerState {
        workflow_path: "workflow.yaml".to_string(),
        working_dir: ".".to_string(),
    };

    let server = tokio::spawn(async move {
        run_with_shutdown(listener, state, None, async move {
            let _ = shutdown_rx.await;
        })
        .await
    });

    // Retry briefly until the server starts listening.
    let client = reqwest::Client::new();
    let response = loop {
        match client.post(format!("http://{addr}/health")).send().await {
            Ok(resp) => break resp,
            Err(_) => sleep(Duration::from_millis(50)).await,
        }
    };
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = response.json().await.expect("health json");
    assert_eq!(body["status"], "healthy");

    let _ = shutdown_tx.send(());
    let result = server.await.expect("join server");
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_run_from_args_serves_health_route() {
    init_tracing();
    let addr = free_local_addr();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let args = Args {
        addr: addr.to_string(),
        workflow_path: "workflow.yaml".to_string(),
        working_dir: ".".to_string(),
        auth_token: None,
    };

    let server = tokio::spawn(async move {
        run_from_args(args, async move {
            let _ = shutdown_rx.await;
        })
        .await
    });

    let client = reqwest::Client::new();
    let response = loop {
        match client.post(format!("http://{addr}/health")).send().await {
            Ok(resp) => break resp,
            Err(_) => sleep(Duration::from_millis(50)).await,
        }
    };
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let _ = shutdown_tx.send(());
    let result = server.await.expect("join server");
    assert!(result.is_ok());
}

fn free_local_addr() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
    listener.local_addr().expect("local addr")
}
