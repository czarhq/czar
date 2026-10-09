use super::*;
use czar_core::job_tracker::{JobStatus, JobTracker};
use serde_json::json;
use tokio::time::{Duration, timeout};
use uuid::Uuid;

#[test]
fn test_cloud_tracker_service_types() {
    assert_eq!(BigQueryTracker::without_client().service_type(), "bigquery");
    assert_eq!(CloudRunTracker::without_client().service_type(), "cloudrun");
    assert_eq!(DataprocTracker::without_client().service_type(), "dataproc");
}

#[tokio::test]
async fn test_bigquery_tracker_validates_inputs_and_client_presence() {
    let tracker = BigQueryTracker::without_client();

    let missing_project = tracker
        .poll_job("job", &json!({}))
        .await
        .expect_err("missing project");
    assert!(missing_project.to_string().contains("Missing project"));

    let missing_location = tracker
        .poll_job("job", &json!({"project":"p"}))
        .await
        .expect_err("missing location");
    assert!(missing_location.to_string().contains("Missing location"));

    let missing_job_id = tracker
        .poll_job("job", &json!({"project":"p","location":"us"}))
        .await
        .expect_err("missing job_id");
    assert!(missing_job_id.to_string().contains("Missing job_id"));

    let missing_client = tracker
        .poll_job("job", &json!({"project":"p","location":"us","job_id":"x"}))
        .await
        .expect_err("missing client");
    assert!(
        missing_client
            .to_string()
            .contains("client not initialized")
    );
}

#[tokio::test]
async fn test_cloudrun_tracker_validates_inputs_and_client_presence() {
    let tracker = CloudRunTracker::without_client();

    let missing_project = tracker
        .poll_job("job", &json!({}))
        .await
        .expect_err("missing project");
    assert!(missing_project.to_string().contains("Missing project"));

    let missing_region = tracker
        .poll_job("job", &json!({"project":"p"}))
        .await
        .expect_err("missing region");
    assert!(missing_region.to_string().contains("Missing region"));

    let missing_job_name = tracker
        .poll_job("job", &json!({"project":"p","region":"us"}))
        .await
        .expect_err("missing job_name");
    assert!(missing_job_name.to_string().contains("Missing job_name"));

    let missing_execution_id = tracker
        .poll_job("job", &json!({"project":"p","region":"us","job_name":"n"}))
        .await
        .expect_err("missing execution_id");
    assert!(
        missing_execution_id
            .to_string()
            .contains("Missing execution_id")
    );

    let missing_client = tracker
        .poll_job(
            "job",
            &json!({
                "project":"p",
                "region":"us",
                "job_name":"n",
                "execution_id":"e"
            }),
        )
        .await
        .expect_err("missing client");
    assert!(
        missing_client
            .to_string()
            .contains("client not initialized")
    );
}

#[tokio::test]
async fn test_dataproc_tracker_validates_inputs_and_client_presence() {
    let tracker = DataprocTracker::without_client();

    let missing_project = tracker
        .poll_job("job", &json!({}))
        .await
        .expect_err("missing project");
    assert!(missing_project.to_string().contains("Missing project"));

    let missing_region = tracker
        .poll_job("job", &json!({"project":"p"}))
        .await
        .expect_err("missing region");
    assert!(missing_region.to_string().contains("Missing region"));

    let missing_cluster_name = tracker
        .poll_job("job", &json!({"project":"p","region":"us"}))
        .await
        .expect_err("missing cluster_name");
    assert!(
        missing_cluster_name
            .to_string()
            .contains("Missing cluster_name")
    );

    let missing_job_id = tracker
        .poll_job(
            "job",
            &json!({"project":"p","region":"us","cluster_name":"cluster"}),
        )
        .await
        .expect_err("missing job_id");
    assert!(missing_job_id.to_string().contains("Missing job_id"));

    let missing_client = tracker
        .poll_job(
            "job",
            &json!({
                "project":"p",
                "region":"us",
                "cluster_name":"cluster",
                "job_id":"job-1"
            }),
        )
        .await
        .expect_err("missing client");
    assert!(
        missing_client
            .to_string()
            .contains("client not initialized")
    );
}

#[test]
fn test_map_bigquery_status_transitions_and_default_error_message() {
    let running: BigQueryJobStatus =
        serde_json::from_value(json!({"state": "RUNNING"})).expect("running status");
    assert_eq!(map_bigquery_status(&running), JobStatus::Running);

    let completed: BigQueryJobStatus =
        serde_json::from_value(json!({"state": "DONE"})).expect("completed status");
    assert_eq!(map_bigquery_status(&completed), JobStatus::Completed);

    let failed_with_missing_message: BigQueryJobStatus = serde_json::from_value(json!({
        "state": "DONE",
        "errorResult": {}
    }))
    .expect("failed status");
    assert!(matches!(
        map_bigquery_status(&failed_with_missing_message),
        JobStatus::Failed(msg) if msg.contains("Unknown error")
    ));
}

#[test]
fn test_map_cloudrun_execution_conditions() {
    let running: CloudRunExecutionResponse = serde_json::from_value(json!({
        "conditions": [{"type": "Completed", "state": "CONDITION_RECONCILING"}]
    }))
    .expect("running execution");
    assert_eq!(map_cloudrun_execution(&running), JobStatus::Running);

    let completed: CloudRunExecutionResponse = serde_json::from_value(json!({
        "conditions": [{"type": "Completed", "state": "CONDITION_SUCCEEDED"}]
    }))
    .expect("completed execution");
    assert_eq!(map_cloudrun_execution(&completed), JobStatus::Completed);

    let failed: CloudRunExecutionResponse = serde_json::from_value(json!({
        "conditions": [{"type": "Ready", "state": "CONDITION_FAILED"}]
    }))
    .expect("failed execution");
    assert!(matches!(
        map_cloudrun_execution(&failed),
        JobStatus::Failed(msg) if msg.contains("Execution failed")
    ));
}

#[test]
fn test_map_dataproc_job_statuses() {
    let running: DataprocJobResponse = serde_json::from_value(json!({
        "status": {"state": "RUNNING"}
    }))
    .expect("running job");
    assert_eq!(map_dataproc_job(&running), JobStatus::Running);

    let completed: DataprocJobResponse = serde_json::from_value(json!({
        "status": {"state": "DONE"}
    }))
    .expect("completed job");
    assert_eq!(map_dataproc_job(&completed), JobStatus::Completed);

    let failed: DataprocJobResponse = serde_json::from_value(json!({
        "status": {"state": "ERROR", "details": "cluster failed"}
    }))
    .expect("failed job");
    assert!(matches!(
        map_dataproc_job(&failed),
        JobStatus::Failed(msg) if msg.contains("cluster failed")
    ));
}

#[tokio::test]
async fn test_cloud_tracker_constructors_do_not_require_credentials_eagerly() {
    let prev_creds = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok();
    let bad_path = std::env::temp_dir().join(format!("czar-creds-{}.json", Uuid::new_v4()));
    unsafe {
        std::env::set_var(
            "GOOGLE_APPLICATION_CREDENTIALS",
            bad_path.to_string_lossy().to_string(),
        );
    }

    let cloud_run = timeout(Duration::from_secs(5), CloudRunTracker::new())
        .await
        .expect("cloud run constructor should not hang");
    let _ = cloud_run;

    let dataproc = timeout(Duration::from_secs(5), DataprocTracker::new())
        .await
        .expect("dataproc constructor should not hang");
    let _ = dataproc;

    match prev_creds {
        Some(v) => unsafe { std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS", v) },
        None => unsafe { std::env::remove_var("GOOGLE_APPLICATION_CREDENTIALS") },
    }
}
