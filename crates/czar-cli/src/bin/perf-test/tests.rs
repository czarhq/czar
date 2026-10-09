use super::*;

#[test]
fn test_default_db_pool_size() {
    assert_eq!(default_db_pool_size(), 10);
}

#[test]
fn test_perf_config_deserialization() {
    let yaml = r#"
workflows: 2
tasks_per_workflow: 3
duration: 0.5
scheduler:
  poll_interval_secs: 0.1
  max_tasks_per_batch: 100
  max_concurrent_dispatches: 10
  max_concurrent_status_checks: 20
"#;

    let cfg: PerfConfig = serde_yaml::from_str(yaml).expect("config should deserialize");
    assert_eq!(cfg.workflows, 2);
    assert_eq!(cfg.tasks_per_workflow, 3);
    assert_eq!(cfg.scheduler.db_pool_size, 10);
    assert_eq!(
        cfg.db_extra_latency_ms, 0,
        "default should be zero (no latency)"
    );
}

#[test]
fn test_perf_config_with_db_latency() {
    let yaml = r#"
workflows: 10
tasks_per_workflow: 2
duration: 0.1
db_extra_latency_ms: 5
scheduler:
  poll_interval_secs: 1.0
  max_tasks_per_batch: 50
  max_concurrent_dispatches: 10
  max_concurrent_status_checks: 10
"#;

    let cfg: PerfConfig = serde_yaml::from_str(yaml).expect("config should deserialize");
    assert_eq!(cfg.db_extra_latency_ms, 5);
}

#[test]
fn test_scheduler_metrics_deserialization() {
    let value = serde_json::json!({
        "timestamp": 123,
        "process_pending_runs_ms": 10,
        "process_pending_tasks_ms": 20,
        "process_status_updates_ms": 30,
        "sleep_ms": 40,
        "total_loop_ms": 100
    });
    let parsed: SchedulerMetrics =
        serde_json::from_value(value).expect("metrics should deserialize");
    assert_eq!(parsed.process_pending_runs_ms, 10);
    assert_eq!(parsed.total_loop_ms, 100);
}
