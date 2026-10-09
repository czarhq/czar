//! Triggerer component for tracking deferred jobs
//!
//! The Triggerer is a sub-component within the scheduler process that polls
//! external APIs to track long-running jobs (BigQuery, Cloud Run, Dataproc, etc.)
//!
//! Key responsibilities:
//! - Poll external APIs for job status
//! - Update deferred job status in database
//! - Notify scheduler when jobs complete
//! - Handle failures and retries

use crate::database::Database;
use crate::job_tracker::{JobStatus, JobTracker};
use crate::models::{DeferredJob, DeferredJobStatus};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Message sent from Triggerer to Scheduler when a job completes
#[derive(Debug, Clone, Default)]
pub struct JobCompletionNotification {
    /// Task ID that owns this deferred job
    pub task_id: Uuid,
    /// Whether the job completed successfully
    pub success: bool,
    /// Error message if job failed
    pub error: Option<String>,
}

/// Configuration for the Triggerer component
#[derive(Debug, Clone)]
pub struct TriggererConfig {
    /// Interval between polling cycles (default: 10 seconds)
    pub poll_interval: Duration,
    /// Maximum number of jobs to poll per cycle (default: 100)
    pub max_jobs_per_cycle: usize,
    /// Timeout for individual job poll requests (default: 30 seconds)
    pub poll_timeout: Duration,
}

impl Default for TriggererConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(10),
            max_jobs_per_cycle: 100,
            poll_timeout: Duration::from_secs(30),
        }
    }
}

/// Triggerer component that polls external APIs for deferred job status
///
/// The Triggerer runs in the background, continuously polling external APIs
/// to check the status of deferred jobs. When a job completes (or fails),
/// it notifies the scheduler via a channel.
pub struct Triggerer<D: Database + 'static> {
    db: Arc<D>,
    trackers: HashMap<String, Arc<dyn JobTracker>>,
    config: TriggererConfig,
    completion_tx: mpsc::UnboundedSender<JobCompletionNotification>,
}

impl<D> Default for Triggerer<D>
where
    D: Database + Default + 'static,
{
    fn default() -> Self {
        let (completion_tx, _completion_rx) = mpsc::unbounded_channel();
        Self {
            db: Arc::new(D::default()),
            trackers: HashMap::new(),
            config: TriggererConfig::default(),
            completion_tx,
        }
    }
}

impl<D: Database> Triggerer<D> {
    /// Create a new Triggerer instance
    ///
    /// # Arguments
    /// * `db` - Database instance for tracking job state
    /// * `completion_tx` - Channel to send job completion notifications to scheduler
    pub fn new(
        db: Arc<D>,
        completion_tx: mpsc::UnboundedSender<JobCompletionNotification>,
    ) -> Self {
        Self::with_config(db, completion_tx, TriggererConfig::default())
    }

    /// Create a new Triggerer with custom configuration
    pub fn with_config(
        db: Arc<D>,
        completion_tx: mpsc::UnboundedSender<JobCompletionNotification>,
        config: TriggererConfig,
    ) -> Self {
        Self {
            db,
            trackers: HashMap::new(),
            config,
            completion_tx,
        }
    }

    /// Register a job tracker for a service type
    pub fn register_tracker(&mut self, tracker: Arc<dyn JobTracker>) {
        let service_type = tracker.service_type().to_string();
        self.trackers.insert(service_type, tracker);
    }

    /// Start the triggerer polling loop
    ///
    /// Returns a JoinHandle that can be used to wait for the triggerer to stop.
    pub fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    /// Main polling loop
    async fn run(self) {
        tracing::info!("Triggerer started");

        let mut interval = tokio::time::interval(self.config.poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if let Err(e) = self.poll_cycle().await {
                tracing::error!("Error in triggerer poll cycle: {}", e);
            }
        }
    }

    /// Perform one polling cycle
    async fn poll_cycle(&self) -> Result<()> {
        // Get pending deferred jobs from database
        let jobs = self.db.get_pending_deferred_jobs().await?;

        if jobs.is_empty() {
            tracing::trace!("No deferred jobs to poll");
            return Ok(());
        }

        tracing::debug!("Polling {} deferred jobs", jobs.len());

        // Limit number of jobs per cycle
        let jobs_to_poll = jobs
            .into_iter()
            .take(self.config.max_jobs_per_cycle)
            .collect::<Vec<_>>();

        // Poll jobs concurrently
        let poll_tasks: Vec<_> = jobs_to_poll
            .into_iter()
            .map(|job| {
                let db = Arc::clone(&self.db);
                let trackers = self.trackers.clone();
                let completion_tx = self.completion_tx.clone();
                let timeout = self.config.poll_timeout;

                tokio::spawn(async move {
                    Self::poll_job(db, trackers, completion_tx, job, timeout).await
                })
            })
            .collect();

        // Wait for all polls to complete
        let results = futures::future::join_all(poll_tasks).await;

        // Log any errors
        for result in results {
            if let Err(e) = result {
                tracing::error!("Error polling deferred job: {}", e);
            }
        }

        Ok(())
    }

    /// Poll a single deferred job
    async fn poll_job(
        db: Arc<D>,
        trackers: HashMap<String, Arc<dyn JobTracker>>,
        completion_tx: mpsc::UnboundedSender<JobCompletionNotification>,
        job: DeferredJob,
        timeout: Duration,
    ) -> Result<()> {
        // Get the appropriate tracker for this job
        let tracker = trackers
            .get(&job.service_type)
            .ok_or_else(|| anyhow::anyhow!("No tracker for service type: {}", job.service_type))?;

        // Poll the job with timeout
        let poll_result = tokio::time::timeout(
            timeout,
            tracker.poll_job(&job.job_id, crate::models::json_inner(&job.job_data)),
        )
        .await;

        // Update last polled timestamp
        if let Err(e) = db.update_deferred_job_polled(job.id).await {
            tracing::warn!("Failed to update last_polled_at for job {}: {}", job.id, e);
        }

        match poll_result {
            Ok(Ok(JobStatus::Running)) => {
                // Job still running, update status to polling if needed
                if job.status() != DeferredJobStatus::Polling {
                    db.update_deferred_job_status(job.id, DeferredJobStatus::Polling, None)
                        .await?;
                }
                tracing::trace!("Job {} still running", job.job_id);
            }
            Ok(Ok(JobStatus::Completed)) => {
                // Job completed successfully
                tracing::info!("Job {} completed successfully", job.job_id);
                db.complete_deferred_job(job.id).await?;

                // Notify scheduler
                let _ = completion_tx.send(JobCompletionNotification {
                    task_id: job.task_id,
                    success: true,
                    error: None,
                });
            }
            Ok(Ok(JobStatus::Failed(error))) => {
                // Job failed
                tracing::warn!("Job {} failed: {}", job.job_id, error);
                db.fail_deferred_job(job.id, &error).await?;

                // Notify scheduler
                let _ = completion_tx.send(JobCompletionNotification {
                    task_id: job.task_id,
                    success: false,
                    error: Some(error),
                });
            }
            Ok(Err(e)) => {
                // Error polling job
                tracing::error!("Error polling job {}: {}", job.job_id, e);
                db.fail_deferred_job(job.id, &e.to_string()).await?;

                // Notify scheduler
                let _ = completion_tx.send(JobCompletionNotification {
                    task_id: job.task_id,
                    success: false,
                    error: Some(e.to_string()),
                });
            }
            Err(_) => {
                // Timeout
                tracing::warn!("Timeout polling job {}", job.job_id);
                // Don't fail the job on timeout, just log and continue
                // It will be polled again in the next cycle
            }
        }

        Ok(())
    }

    /// Notify the triggerer about a new deferred job
    ///
    /// This is called by the scheduler when a task returns a deferrable.
    /// Note: The triggerer will pick up the job automatically in the next
    /// polling cycle, so this is mainly for logging/metrics purposes.
    pub fn notify_new_job(&self, _task_id: Uuid, _job_id: &str, _service_type: &str) {
        // In the current implementation, the triggerer polls the database
        // for pending jobs, so we don't need to do anything here.
        // In a future optimization, we could use a notification channel
        // to immediately start polling new jobs without waiting for the
        // next cycle.
        tracing::trace!("New deferred job notification received");
    }
}

#[cfg(test)]
mod tests;
