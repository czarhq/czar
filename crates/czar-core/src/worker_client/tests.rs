use super::*;
use axum::{Json, Router, http::StatusCode, routing::post};
use serde_json::json;
use std::net::SocketAddr;
use tokio::net::TcpListener;

async fn spawn_test_server(app: Router) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("listener addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve test app");
    });
    addr
}

#[test]
fn test_worker_client_creation() {
    let client = WorkerClient::new("http://localhost:8081".to_string());
    assert_eq!(client.worker_url, "http://localhost:8081");
}

#[test]
fn test_compile_request_skips_missing_path() {
    let req = CompileRequest::default();
    let value = serde_json::to_value(req).expect("compile request serialization should succeed");
    assert!(value.get("workflow_path").is_none());
}

#[test]
fn test_execute_response_deserialization_variants() {
    let success: ExecuteResponse = serde_json::from_value(json!({
        "status": "success",
        "output": { "ok": true }
    }))
    .expect("success execute response should deserialize");
    assert!(matches!(success, ExecuteResponse::Success { .. }));

    let failed: ExecuteResponse = serde_json::from_value(json!({
        "status": "failed",
        "error": "boom"
    }))
    .expect("failed execute response should deserialize");
    assert!(matches!(failed, ExecuteResponse::Failed { .. }));
}

#[tokio::test]
async fn test_compile_success_and_error_paths() {
    let success_app = Router::new().route(
        "/compile",
        post(|| async {
            (
                StatusCode::OK,
                Json(json!({
                    "workflow": {"name":"wf"},
                    "tasks": [{"name":"t1","executor":"process","depends_on":[]}]
                })),
            )
        }),
    );
    let success_addr = spawn_test_server(success_app).await;
    let success_client = WorkerClient::new(format!("http://{}", success_addr));
    let ok = success_client
        .compile(Some("workflow.yaml".to_string()))
        .await
        .expect("compile request should succeed");
    assert_eq!(ok.tasks.len(), 1);
    assert_eq!(ok.tasks[0].name, "t1");

    let error_app = Router::new().route(
        "/compile",
        post(|| async { (StatusCode::BAD_REQUEST, "bad request") }),
    );
    let error_addr = spawn_test_server(error_app).await;
    let failing_client = WorkerClient::new(format!("http://{}", error_addr));
    let err = failing_client
        .compile(None)
        .await
        .expect_err("compile should fail");
    assert!(
        err.to_string()
            .contains("Compile request failed with status")
    );
}

#[tokio::test]
async fn test_execute_success_and_failed_status_response() {
    let app = Router::new()
        .route(
            "/execute",
            post(|| async {
                (
                    StatusCode::OK,
                    Json(json!({
                        "status": "success",
                        "output": {"ok": true},
                        "deferred": []
                    })),
                )
            }),
        )
        .route("/health", post(|| async { (StatusCode::OK, "ok") }));
    let addr = spawn_test_server(app).await;
    let client = WorkerClient::new(format!("http://{}", addr));
    let req = ExecuteRequest {
        task_id: Uuid::new_v4(),
        params: Some(json!({"command": "echo hi"})),
        ..Default::default()
    };

    let resp = client.execute(req).await.expect("execute should succeed");
    assert!(matches!(resp, ExecuteResponse::Success { .. }));
    client
        .health_check()
        .await
        .expect("health check should succeed");
}

#[tokio::test]
async fn test_execute_and_health_check_http_error() {
    let app = Router::new()
        .route(
            "/execute",
            post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
        )
        .route(
            "/health",
            post(|| async { (StatusCode::SERVICE_UNAVAILABLE, "down") }),
        );
    let addr = spawn_test_server(app).await;
    let client = WorkerClient::new(format!("http://{}", addr));

    let req = ExecuteRequest {
        task_id: Uuid::new_v4(),
        ..Default::default()
    };
    let execute_err = client
        .execute(req)
        .await
        .expect_err("execute should fail on 500");
    assert!(
        execute_err
            .to_string()
            .contains("Execute request failed with status")
    );

    let health_err = client
        .health_check()
        .await
        .expect_err("health should fail on non-success");
    assert!(
        health_err
            .to_string()
            .contains("Health check failed with status")
    );
}

#[tokio::test]
async fn test_token_is_sent_as_bearer_header() {
    let app = Router::new().route(
        "/health",
        post(|headers: axum::http::HeaderMap| async move {
            match headers.get("authorization").and_then(|v| v.to_str().ok()) {
                Some("Bearer secret") => StatusCode::OK,
                _ => StatusCode::UNAUTHORIZED,
            }
        }),
    );
    let addr = spawn_test_server(app).await;

    let client = WorkerClient::new(format!("http://{}", addr));
    assert!(client.health_check().await.is_err());
    let client = client.with_token(Some("secret".to_string()));
    client
        .health_check()
        .await
        .expect("health check with token should succeed");
}
