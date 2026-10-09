use super::*;
use chrono::Utc;
use czar_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use czar_core::models::Workflow;

fn sample_workflow() -> Workflow {
    Workflow {
        id: uuid::Uuid::new_v4(),
        name: "wf".to_string(),
        description: Some("workflow".to_string()),
        job_name: "job".to_string(),
        region: "local".to_string(),
        project: "local".to_string(),
        executor_type: "process".to_string(),
        task_params: None,
        schedule: None,
        schedule_enabled: false,
        last_scheduled_at: None,
        next_scheduled_at: None,
        current_snapshot_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn test_get_executor_for_enabled_types() {
    let manager = ExecutorManager::new();
    let workflow = sample_workflow();

    assert!(manager.get_executor("process", &workflow).await.is_ok());
    assert!(manager.get_executor("container", &workflow).await.is_ok());
    assert!(manager.get_executor("python", &workflow).await.is_ok());
    assert!(manager.get_executor("library", &workflow).await.is_ok());
    assert!(manager.get_executor("wasm", &workflow).await.is_ok());
}

#[tokio::test]
async fn test_get_executor_rejects_unknown_or_disabled_types() {
    let manager = ExecutorManager::new();
    let workflow = sample_workflow();

    let unknown_err = manager
        .get_executor("does-not-exist", &workflow)
        .await
        .err()
        .expect("unknown executor should fail");
    assert!(unknown_err.to_string().contains("Unknown executor type"));

    let cloudrun_err = manager
        .get_executor("cloudrun", &workflow)
        .await
        .err()
        .expect("cloudrun should fail when feature disabled");
    assert!(
        cloudrun_err
            .to_string()
            .contains("CloudRun executor is unavailable on this build")
    );
}

#[cfg(feature = "worker")]
#[test]
fn test_with_workers_keeps_tokens_by_url() {
    let worker = |url: &str, token: Option<&str>| WorkerConfig {
        url: url.to_string(),
        name: None,
        poll_interval_secs: None,
        token: token.map(str::to_string),
        token_env: None,
    };
    let manager = ExecutorManager::new().with_workers(&[
        worker("http://a:8081", Some("secret")),
        worker("http://b:8081", None),
    ]);

    assert_eq!(
        manager
            .worker_tokens
            .get("http://a:8081")
            .map(String::as_str),
        Some("secret")
    );
    assert!(!manager.worker_tokens.contains_key("http://b:8081"));
}
