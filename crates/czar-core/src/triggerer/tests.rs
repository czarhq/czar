use super::*;

#[test]
fn test_default_config() {
    let config = TriggererConfig::default();
    assert_eq!(config.poll_interval, Duration::from_secs(10));
    assert_eq!(config.max_jobs_per_cycle, 100);
    assert_eq!(config.poll_timeout, Duration::from_secs(30));
}

#[test]
fn test_job_completion_notification() {
    let notification = JobCompletionNotification {
        task_id: Uuid::new_v4(),
        success: true,
        error: None,
    };
    assert!(notification.success);
    assert!(notification.error.is_none());
}
