use super::*;

#[derive(Debug, Default)]
struct MockDeferrable {
    payload: serde_json::Value,
}

impl Deferrable for MockDeferrable {
    fn service_type(&self) -> &str {
        "mock"
    }

    fn job_id(&self) -> String {
        "mock-job".to_string()
    }

    fn to_json(&self) -> serde_json::Value {
        self.payload.clone()
    }
}

#[test]
fn test_bigquery_job_service_type() {
    let job = BigQueryJob {
        project: "test-project".to_string(),
        job_id: "job123".to_string(),
        location: "US".to_string(),
    };
    assert_eq!(job.service_type(), "bigquery");
    assert_eq!(job.job_id(), "test-project/US/job123");
}

#[test]
fn test_cloudrun_job_service_type() {
    let job = CloudRunJob {
        project: "test-project".to_string(),
        region: "us-central1".to_string(),
        job_name: "my-job".to_string(),
        execution_id: "exec123".to_string(),
    };
    assert_eq!(job.service_type(), "cloudrun");
    assert_eq!(job.job_id(), "test-project/us-central1/my-job/exec123");
}

#[test]
fn test_dataproc_job_service_type() {
    let job = DataprocJob {
        project: "test-project".to_string(),
        region: "us-central1".to_string(),
        cluster_name: "my-cluster".to_string(),
        job_id: "job123".to_string(),
    };
    assert_eq!(job.service_type(), "dataproc");
    assert_eq!(job.job_id(), "test-project/us-central1/my-cluster/job123");
}

#[test]
fn test_custom_http_service_type() {
    let job = CustomHttp {
        url: "https://api.example.com/status".to_string(),
        method: "GET".to_string(),
        headers: HashMap::new(),
        success_status_codes: vec![200],
        completion_field: "status".to_string(),
        completion_value: "done".to_string(),
        failure_value: Some("failed".to_string()),
    };
    assert_eq!(job.service_type(), "custom_http");
    assert_eq!(job.job_id(), "https://api.example.com/status");
    assert_eq!(
        job.to_json(),
        serde_json::json!({
            "url":"https://api.example.com/status",
            "method":"GET",
            "headers":{},
            "success_status_codes":[200],
            "completion_field":"status",
            "completion_value":"done",
            "failure_value":"failed"
        })
    );
}

#[test]
fn test_task_result_serialization() {
    let result: TaskResult<String> = TaskResult::data("hello".to_string());
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("hello"));
}

#[test]
fn test_custom_http_defaults_success_codes_when_missing() {
    let parsed: CustomHttp = serde_json::from_value(serde_json::json!({
        "url": "https://api.example.com/status",
        "method": "GET",
        "completion_field": "state",
        "completion_value": "done"
    }))
    .expect("custom http should deserialize");

    assert_eq!(parsed.success_status_codes, vec![200]);
    assert!(parsed.headers.is_empty());
}

#[test]
fn test_deferrable_to_json_round_trips_for_builtins() {
    let bq = BigQueryJob {
        project: "p".to_string(),
        job_id: "j".to_string(),
        location: "us".to_string(),
    };
    assert_eq!(
        bq.to_json(),
        serde_json::json!({"project":"p","job_id":"j","location":"us"})
    );

    let cloud = CloudRunJob {
        project: "p".to_string(),
        region: "r".to_string(),
        job_name: "n".to_string(),
        execution_id: "e".to_string(),
    };
    assert_eq!(
        cloud.to_json(),
        serde_json::json!({"project":"p","region":"r","job_name":"n","execution_id":"e"})
    );

    let dp = DataprocJob {
        project: "p".to_string(),
        region: "r".to_string(),
        cluster_name: "c".to_string(),
        job_id: "j".to_string(),
    };
    assert_eq!(
        dp.to_json(),
        serde_json::json!({"project":"p","region":"r","cluster_name":"c","job_id":"j"})
    );
}

#[test]
fn test_task_result_deferred_and_mixed_helpers() {
    let mock = MockDeferrable {
        payload: serde_json::json!({"service_type":"mock","job_id":"job-0"}),
    };
    assert_eq!(mock.service_type(), "mock");
    assert_eq!(mock.job_id(), "mock-job");

    let deferred_only: TaskResult<String> = TaskResult::deferred(vec![Box::new(MockDeferrable {
        payload: serde_json::json!({"service_type":"mock","job_id":"job-1"}),
    })]);
    let TaskResult::Deferred { deferred } = deferred_only else {
        panic!("expected deferred variant");
    };
    assert_eq!(deferred.len(), 1);
    assert_eq!(deferred[0]["service_type"], "mock");

    let mixed = TaskResult::mixed(
        "data".to_string(),
        vec![Box::new(MockDeferrable {
            payload: serde_json::json!({"service_type":"mock","job_id":"job-2"}),
        })],
    );
    let TaskResult::Mixed { data, deferred } = mixed else {
        panic!("expected mixed variant");
    };
    assert_eq!(data, "data");
    assert_eq!(deferred.len(), 1);
    assert_eq!(deferred[0]["job_id"], "job-2");
}
