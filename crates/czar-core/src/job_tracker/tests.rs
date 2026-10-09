use super::*;
use axum::http::Method;
use axum::{Json, Router, routing::any};
use serde_json::json;
use std::net::SocketAddr;
use tokio::net::TcpListener;

async fn spawn_json_server(status: u16, body: serde_json::Value) -> SocketAddr {
    let app = Router::new().route(
        "/status",
        any(move || {
            let body = body.clone();
            async move {
                (
                    axum::http::StatusCode::from_u16(status).expect("valid status"),
                    Json(body),
                )
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve app");
    });
    addr
}

async fn spawn_method_header_server() -> SocketAddr {
    let app = Router::new().route(
        "/status",
        any(
            |method: Method, headers: axum::http::HeaderMap| async move {
                let authorized = headers
                    .get("x-test")
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v == "1")
                    .unwrap_or(false);
                let state = if authorized { "done" } else { "running" };
                (
                    axum::http::StatusCode::OK,
                    Json(json!({
                        "state": state,
                        "method": method.as_str(),
                    })),
                )
            },
        ),
    );
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve app");
    });
    addr
}

#[test]
fn test_custom_http_tracker_service_type() {
    let tracker = CustomHttpTracker::new();
    assert_eq!(tracker.service_type(), "custom_http");
}

#[test]
fn test_custom_http_tracker_default_constructs_tracker() {
    let tracker = CustomHttpTracker::default();
    assert_eq!(tracker.service_type(), "custom_http");
}

#[tokio::test]
async fn test_custom_http_validates_url() {
    let tracker = CustomHttpTracker::new();
    let invalid_data = json!({});
    let result = tracker.poll_job("test-job", &invalid_data).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Missing url"));
}

#[tokio::test]
async fn test_custom_http_validates_status_field_default() {
    let tracker = CustomHttpTracker::new();
    let addr = spawn_json_server(200, json!({"state": "done"})).await;
    let result = tracker
        .poll_job(
            "test-job",
            &json!({
                "url": format!("http://{}/status", addr)
            }),
        )
        .await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("missing expected field 'status'")
    );
}

#[tokio::test]
async fn test_custom_http_defaults_success_value() {
    let tracker = CustomHttpTracker::new();
    let addr = spawn_json_server(200, json!({"status": "done"})).await;
    let result = tracker
        .poll_job(
            "test-job",
            &json!({
                "url": format!("http://{}/status", addr)
            }),
        )
        .await
        .expect("poll should succeed");
    assert_eq!(result, JobStatus::Running);
}

#[tokio::test]
async fn test_custom_http_completed_running_and_failed_statuses() {
    let tracker = CustomHttpTracker::new();
    let addr = spawn_json_server(200, json!({"state": "done"})).await;
    let completed = tracker
        .poll_job(
            "job",
            &json!({
                "url": format!("http://{}/status", addr),
                "status_field": "state",
                "success_value": "done",
                "failure_value": "failed"
            }),
        )
        .await
        .expect("poll should succeed");
    assert_eq!(completed, JobStatus::Completed);

    let addr = spawn_json_server(200, json!({"state": "running"})).await;
    let running = tracker
        .poll_job(
            "job",
            &json!({
                "url": format!("http://{}/status", addr),
                "status_field": "state",
                "success_value": "done",
                "failure_value": "failed"
            }),
        )
        .await
        .expect("poll should succeed");
    assert_eq!(running, JobStatus::Running);

    let addr = spawn_json_server(200, json!({"state": "failed"})).await;
    let failed = tracker
        .poll_job(
            "job",
            &json!({
                "url": format!("http://{}/status", addr),
                "status_field": "state",
                "success_value": "done",
                "failure_value": "failed"
            }),
        )
        .await
        .expect("poll should succeed");
    assert!(matches!(failed, JobStatus::Failed(msg) if msg.contains("failed")));
}

#[tokio::test]
async fn test_custom_http_handles_http_status_method_and_response_shape_errors() {
    let tracker = CustomHttpTracker::new();

    let bad_status_addr = spawn_json_server(503, json!({"state": "done"})).await;
    let bad_status = tracker
        .poll_job(
            "job",
            &json!({
                "url": format!("http://{}/status", bad_status_addr),
                "status_field": "state",
                "success_value": "done"
            }),
        )
        .await
        .expect("poll should return job status on bad http code");
    assert!(matches!(bad_status, JobStatus::Failed(msg) if msg.contains("503")));

    let unsupported_method = tracker
        .poll_job(
            "job",
            &json!({
                "url": "http://127.0.0.1:1/status",
                "method": "PATCH",
                "status_field": "state",
                "success_value": "done"
            }),
        )
        .await
        .expect_err("unsupported method should error");
    assert!(
        unsupported_method
            .to_string()
            .contains("Unsupported HTTP method")
    );

    let bad_body_addr = spawn_json_server(200, json!({"other_field": "x"})).await;
    let bad_body = tracker
        .poll_job(
            "job",
            &json!({
                "url": format!("http://{}/status", bad_body_addr),
                "status_field": "state",
                "success_value": "done",
                "success_status_codes": [200]
            }),
        )
        .await
        .expect_err("missing status field in response should error");
    assert!(bad_body.to_string().contains("missing expected field"));
}

#[tokio::test]
async fn test_custom_http_supports_post_put_delete_and_headers() {
    let tracker = CustomHttpTracker::new();
    let addr = spawn_method_header_server().await;
    let base = format!("http://{}/status", addr);

    let post = tracker
        .poll_job(
            "job",
            &json!({
                "url": base,
                "method": "POST",
                "headers": {"x-test": "1"},
                "status_field": "state",
                "success_value": "done"
            }),
        )
        .await
        .expect("post poll");
    assert_eq!(post, JobStatus::Completed);

    let put = tracker
        .poll_job(
            "job",
            &json!({
                "url": format!("http://{}/status", addr),
                "method": "PUT",
                "headers": {"x-test": "1"},
                "status_field": "state",
                "success_value": "done"
            }),
        )
        .await
        .expect("put poll");
    assert_eq!(put, JobStatus::Completed);

    let delete = tracker
        .poll_job(
            "job",
            &json!({
                "url": format!("http://{}/status", addr),
                "method": "DELETE",
                "headers": {"x-test": "1"},
                "status_field": "state",
                "success_value": "done"
            }),
        )
        .await
        .expect("delete poll");
    assert_eq!(delete, JobStatus::Completed);
}
