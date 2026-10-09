use super::*;

#[test]
fn round_trip_worker_metadata() {
    let meta = WorkerRegistrationMetadata {
        worker_url: "http://worker:8081".to_string(),
    };
    let params = with_worker_metadata(None, &meta);
    let recovered = worker_metadata_from_params(&params).unwrap();
    assert_eq!(recovered, meta);
}

#[test]
fn worker_metadata_preserves_existing_keys() {
    let meta = WorkerRegistrationMetadata {
        worker_url: "http://worker:8081".to_string(),
    };
    let existing = serde_json::json!({"workflow_format": "v2"});
    let params = with_worker_metadata(Some(existing), &meta);
    assert_eq!(params["workflow_format"], "v2");
    assert_eq!(params["worker"]["url"], "http://worker:8081");
}

#[test]
fn parse_workers_config_yaml() {
    let yaml = r#"
workers:
  - url: http://worker1:8081
    name: etl-worker
    poll_interval_secs: 60
    token: secret
  - url: http://worker2:8082
"#;
    let config = WorkersConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.workers.len(), 2);
    assert_eq!(config.workers[0].url, "http://worker1:8081");
    assert_eq!(config.workers[0].name.as_deref(), Some("etl-worker"));
    assert_eq!(config.workers[0].poll_interval_secs, Some(60));
    assert_eq!(config.workers[0].token.as_deref(), Some("secret"));
    assert_eq!(config.workers[1].url, "http://worker2:8082");
    assert!(config.workers[1].poll_interval_secs.is_none());
    assert!(config.workers[1].token.is_none());
}
