use anyhow::Result;
use async_trait::async_trait;
use czar_core::job_tracker::{JobStatus, JobTracker};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Mock job tracker that simulates job completion after a certain number of polls
#[derive(Default)]
struct MockJobTracker {
    service_type: String,
    polls: Arc<Mutex<HashMap<String, usize>>>,
    polls_until_complete: usize,
}

impl MockJobTracker {
    fn new(service_type: &str, polls_until_complete: usize) -> Self {
        Self {
            service_type: service_type.to_string(),
            polls: Arc::new(Mutex::new(HashMap::new())),
            polls_until_complete,
        }
    }
}

#[async_trait]
impl JobTracker for MockJobTracker {
    fn service_type(&self) -> &str {
        &self.service_type
    }

    async fn poll_job(&self, job_id: &str, _job_data: &Value) -> Result<JobStatus> {
        let mut polls = self.polls.lock().unwrap();
        let count = polls.entry(job_id.to_string()).or_insert(0);
        *count += 1;

        if *count >= self.polls_until_complete {
            Ok(JobStatus::Completed)
        } else {
            Ok(JobStatus::Running)
        }
    }
}

#[tokio::test]
async fn test_mock_tracker_completes_after_n_polls() -> Result<()> {
    let tracker = MockJobTracker::new("mock", 3);

    let job_data = json!({"test": "data"});

    // First two polls should return Running
    assert!(matches!(
        tracker.poll_job("job_1", &job_data).await?,
        JobStatus::Running
    ));
    assert!(matches!(
        tracker.poll_job("job_1", &job_data).await?,
        JobStatus::Running
    ));

    // Third poll should return Completed
    assert!(matches!(
        tracker.poll_job("job_1", &job_data).await?,
        JobStatus::Completed
    ));

    Ok(())
}

#[tokio::test]
async fn test_mock_tracker_tracks_separate_jobs() -> Result<()> {
    let tracker = MockJobTracker::new("mock", 2);

    let job_data = json!({});

    // Poll job_1 once
    assert!(matches!(
        tracker.poll_job("job_1", &job_data).await?,
        JobStatus::Running
    ));

    // Poll job_2 once
    assert!(matches!(
        tracker.poll_job("job_2", &job_data).await?,
        JobStatus::Running
    ));

    // Poll job_1 again - should complete
    assert!(matches!(
        tracker.poll_job("job_1", &job_data).await?,
        JobStatus::Completed
    ));

    // Poll job_2 again - should complete
    assert!(matches!(
        tracker.poll_job("job_2", &job_data).await?,
        JobStatus::Completed
    ));

    Ok(())
}
