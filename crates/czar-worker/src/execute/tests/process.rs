use super::*;

#[tokio::test]
async fn test_execute_handler_rejects_unsupported_executor() {
    let value = run_handler(ExecuteRequest {
        executor_type: RequestedExecutorType::Unknown("unknown".to_string()),
        ..Default::default()
    })
    .await;

    assert_eq!(value["status"], "failed");
    assert!(
        value["error"]
            .as_str()
            .expect("error should be string")
            .contains("Unsupported executor type")
    );
}

#[tokio::test]
async fn test_execute_handler_cloudrun_unavailable_without_feature() {
    let value = run_handler(ExecuteRequest {
        executor_type: ExecutorType::CloudRun.into(),
        ..Default::default()
    })
    .await;

    assert_eq!(value["status"], "failed");
    assert!(
        value["error"]
            .as_str()
            .expect("error should be string")
            .contains("not available in this build")
    );
}

#[tokio::test]
async fn test_execute_handler_library_requires_library_path() {
    let value = run_handler(ExecuteRequest {
        executor_type: ExecutorType::Library.into(),
        ..Default::default()
    })
    .await;

    assert_eq!(value["status"], "failed");
    assert!(
        value["error"]
            .as_str()
            .expect("error should be string")
            .contains("Missing library_path")
    );
}

#[tokio::test]
async fn test_execute_handler_process_returns_failed_for_non_json_output() {
    let value = run_handler(ExecuteRequest {
        params: Some(serde_json::json!({
            "command": "echo hello"
        })),
        ..Default::default()
    })
    .await;

    assert_eq!(value["status"], "failed");
    assert!(
        value["error"]
            .as_str()
            .expect("error should be string")
            .contains("without CZAR_OUTPUT JSON")
    );
}

#[tokio::test]
async fn test_execute_handler_process_success_json_output() {
    let value = run_handler(ExecuteRequest {
        params: Some(serde_json::json!({
            "command": "printf 'CZAR_OUTPUT:{\"ok\":true}'"
        })),
        ..Default::default()
    })
    .await;

    assert_eq!(value["status"], "success");
    assert_eq!(value["output"], serde_json::json!({"ok": true}));
    assert!(value.get("deferred").is_none());
}

#[tokio::test]
async fn test_execute_handler_process_success_with_czar_output_prefix_and_deferred() {
    let value = run_handler(ExecuteRequest {
        params: Some(serde_json::json!({
            "command": "printf 'CZAR_OUTPUT:{\"deferred\":[{\"service_type\":\"custom_http\",\"job_id\":\"job-1\",\"url\":\"http://example.com/status\"}]}'"
        })),
        ..Default::default()
    })
    .await;

    assert_eq!(value["status"], "success");
    assert_eq!(
        value["output"]["deferred"][0]["service_type"],
        "custom_http"
    );
    assert_eq!(value["deferred"][0]["job_id"], "job-1");
}
