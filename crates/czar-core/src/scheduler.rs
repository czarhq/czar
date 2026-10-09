use anyhow::Result;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::config::OrchestratorConfig;

use crate::database::Database;

use crate::executor::StatusUpdate;

use crate::executor_manager::ExecutorManager;

use crate::models::{Workflow, WorkflowSnapshot, WorkflowTask};

use crate::task_execution::{build_run_tasks, execute_task, retry_backoff_seconds};

use crate::job_tracker::{CustomHttpTracker, JobTracker};
use crate::triggerer::{JobCompletionNotification, Triggerer, TriggererConfig};

mod dynamic_fanout;
mod processing;
mod status_processing;
mod status_processing_completion;
mod status_processing_hooks;

#[derive(Debug, Default, Serialize)]
pub struct SchedulerMetrics {
    pub timestamp: u64,
    pub process_pending_runs_ms: u128,
    pub process_pending_tasks_ms: u128,
    pub process_status_updates_ms: u128,
    pub sleep_ms: u128,
    pub total_loop_ms: u128,
    pub runs_processed: usize,
    pub tasks_processed: usize,
    pub status_updates_processed: usize,
}

type SnapshotCache = Arc<std::sync::RwLock<HashMap<Uuid, Arc<WorkflowSnapshot>>>>;
type TaskIdentityCache = Arc<std::sync::Mutex<HashMap<Uuid, (Uuid, String)>>>;
type RunMetaCache = Arc<std::sync::Mutex<HashMap<Uuid, (Uuid, Option<Uuid>)>>>;

pub struct Scheduler<D: Database + 'static, E: ExecutorManager + 'static> {
    db: Arc<D>,
    config: OrchestratorConfig,
    executor_manager: Arc<E>,
    status_tx: mpsc::UnboundedSender<StatusUpdate>,
    status_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<StatusUpdate>>>,
    job_completion_tx: mpsc::UnboundedSender<JobCompletionNotification>,
    job_completion_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<JobCompletionNotification>>>,
    /// In-memory cache for immutable workflow snapshots. Populated lazily and never evicted.
    snapshot_cache: SnapshotCache,
    /// Cache: task_id → (run_id, task_name). Populated at dispatch, evicted at terminal state.
    task_identity_cache: TaskIdentityCache,
    /// Cache: run_id → (workflow_id, snapshot_id). Populated when run starts, evicted on completion.
    run_meta_cache: RunMetaCache,
    /// Tracks when timeout enforcement last ran — throttled to avoid per-loop `get_running_tasks` calls.
    last_timeout_check: std::sync::Mutex<Option<Instant>>,
    /// Tracks when scheduled trigger processing last ran — throttled since cron minimum granularity is 1 minute.
    last_schedule_check: std::sync::Mutex<Option<Instant>>,
    external_job_trackers: Vec<Arc<dyn JobTracker>>,
}

impl<D, E> Default for Scheduler<D, E>
where
    D: Database + Default + 'static,
    E: ExecutorManager + Default + 'static,
{
    fn default() -> Self {
        Self::new(Arc::new(D::default()), Arc::new(E::default()))
    }
}

impl<D: Database + 'static, E: ExecutorManager + 'static> Scheduler<D, E> {
    pub fn new(db: Arc<D>, executor_manager: Arc<E>) -> Self {
        Self::new_with_config(db, executor_manager, OrchestratorConfig::default())
    }

    pub fn new_with_config(
        db: Arc<D>,
        executor_manager: Arc<E>,
        config: OrchestratorConfig,
    ) -> Self {
        let (status_tx, status_rx) = mpsc::unbounded_channel();
        let (job_completion_tx, job_completion_rx) = mpsc::unbounded_channel();
        Self {
            db,
            config,
            executor_manager,
            status_tx,
            status_rx: Arc::new(tokio::sync::Mutex::new(status_rx)),
            job_completion_tx,
            job_completion_rx: Arc::new(tokio::sync::Mutex::new(job_completion_rx)),
            snapshot_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
            task_identity_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
            run_meta_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
            last_timeout_check: std::sync::Mutex::new(None),
            last_schedule_check: std::sync::Mutex::new(None),
            external_job_trackers: Vec::new(),
        }
    }

    pub fn with_job_trackers(mut self, trackers: Vec<Arc<dyn JobTracker>>) -> Self {
        self.external_job_trackers = trackers;
        self
    }

    /// Fetch a snapshot, returning a cached copy if already loaded.
    /// Snapshots are immutable once written, so the cache is never invalidated.
    pub(super) async fn get_snapshot_cached(
        &self,
        snapshot_id: Uuid,
    ) -> Result<Arc<WorkflowSnapshot>> {
        {
            let cache = self.snapshot_cache.read().unwrap();
            if let Some(snap) = cache.get(&snapshot_id) {
                return Ok(Arc::clone(snap));
            }
        }
        let snapshot = Arc::new(self.db.get_snapshot(snapshot_id).await?);
        self.snapshot_cache
            .write()
            .unwrap()
            .insert(snapshot_id, Arc::clone(&snapshot));
        Ok(snapshot)
    }

    /// Fetch (run_id, task_name) for a task, returning a cached copy if available.
    pub(super) async fn get_task_identity_cached(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        {
            let cache = self.task_identity_cache.lock().unwrap();
            if let Some(entry) = cache.get(&task_id) {
                return Ok(entry.clone());
            }
        }
        let identity = self.db.get_task_identity(task_id).await?;
        self.task_identity_cache
            .lock()
            .unwrap()
            .insert(task_id, identity.clone());
        Ok(identity)
    }

    /// Fetch (workflow_id, snapshot_id) for a run, returning a cached copy if available.
    /// Runs are immutable after creation so the cache is never invalidated except on completion.
    pub(super) async fn get_run_meta_cached(&self, run_id: Uuid) -> Result<(Uuid, Option<Uuid>)> {
        {
            let cache = self.run_meta_cache.lock().unwrap();
            if let Some(entry) = cache.get(&run_id) {
                return Ok(*entry);
            }
        }
        let run = self.db.get_run(run_id).await?;
        let meta = (run.workflow_id, run.snapshot_id);
        self.run_meta_cache.lock().unwrap().insert(run_id, meta);
        Ok(meta)
    }

    pub(super) fn cache_task_identity(&self, task_id: Uuid, run_id: Uuid, task_name: String) {
        self.task_identity_cache
            .lock()
            .unwrap()
            .insert(task_id, (run_id, task_name));
    }

    pub(super) fn cache_run_meta(
        &self,
        run_id: Uuid,
        workflow_id: Uuid,
        snapshot_id: Option<Uuid>,
    ) {
        self.run_meta_cache
            .lock()
            .unwrap()
            .insert(run_id, (workflow_id, snapshot_id));
    }

    pub(super) fn evict_task_identity(&self, task_id: Uuid) {
        self.task_identity_cache.lock().unwrap().remove(&task_id);
    }

    pub(super) fn evict_run_meta(&self, run_id: Uuid) {
        self.run_meta_cache.lock().unwrap().remove(&run_id);
    }

    /// Enqueue an executor status update for processing in the scheduler loop.
    pub fn enqueue_status_update(&self, update: StatusUpdate) {
        let _ = self.status_tx.send(update);
    }

    /// Enqueue a deferred-job completion event for processing in the scheduler loop.
    pub fn enqueue_job_completion(&self, completion: JobCompletionNotification) {
        let _ = self.job_completion_tx.send(completion);
    }

    /// Start the triggerer component for tracking deferred jobs
    pub async fn start_triggerer(&self) -> tokio::task::JoinHandle<()> {
        // rustls 0.23 requires explicit process-level provider selection when
        // multiple providers are enabled transitively.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let mut triggerer = Triggerer::with_config(
            Arc::clone(&self.db),
            self.job_completion_tx.clone(),
            TriggererConfig::default(),
        );

        for tracker in &self.external_job_trackers {
            triggerer.register_tracker(Arc::clone(tracker));
        }

        // Custom HTTP tracker - always available
        triggerer.register_tracker(Arc::new(CustomHttpTracker::new()));

        triggerer.start()
    }

    pub async fn run(&self) -> Result<()> {
        let startup_msg = format!(
            "Starting scheduler loop (max_batch={}, max_concurrent_dispatch={}, max_concurrent_status={})",
            self.config.max_tasks_per_batch,
            self.config.max_concurrent_dispatches,
            self.config.max_concurrent_status_checks
        );
        info!("{}", startup_msg);

        // Start triggerer in background if enabled
        let _triggerer_handle = if self.config.enable_triggerer {
            Some(self.start_triggerer().await)
        } else {
            info!("Triggerer disabled by config");
            None
        };

        let mut status_rx = self.status_rx.lock().await;
        let mut job_completion_rx = self.job_completion_rx.lock().await;

        loop {
            let loop_start = Instant::now();
            let mut metrics = SchedulerMetrics::default();

            // Process scheduled triggers at most once every 30 seconds.
            // Cron minimum granularity is 1 minute, so this frequency is sufficient
            // while avoiding per-loop `get_due_scheduled_workflows` DB scans.
            let should_check_schedules = {
                let mut last = self.last_schedule_check.lock().unwrap();
                let now = Instant::now();
                let due = last.is_none_or(|t| now.duration_since(t) >= StdDuration::from_secs(30));
                if due {
                    *last = Some(now);
                }
                due
            };
            if should_check_schedules && let Err(e) = self.process_scheduled_triggers().await {
                error!("Error processing scheduled triggers: {}", e);
            }
            // Process pending runs and tasks
            let start = Instant::now();
            let runs_processed = self.process_pending_runs().await.unwrap_or_else(|e| {
                error!("Error processing pending runs: {}", e);
                0
            });
            metrics.process_pending_runs_ms = start.elapsed().as_millis();
            metrics.runs_processed = runs_processed;
            let start = Instant::now();
            let tasks_processed = self.process_pending_tasks().await.unwrap_or_else(|e| {
                error!("Error processing pending tasks: {}", e);
                0
            });
            metrics.process_pending_tasks_ms = start.elapsed().as_millis();
            metrics.tasks_processed = tasks_processed;

            // Process any immediately available status updates
            let start = Instant::now();
            let mut status_updates = Vec::new();
            while let Ok(update) = status_rx.try_recv() {
                status_updates.push(update);
            }
            let updates_processed = status_updates.len();
            if !status_updates.is_empty()
                && let Err(e) = self.process_status_updates(status_updates).await
            {
                error!("Error processing status updates: {}", e);
            }
            metrics.process_status_updates_ms = start.elapsed().as_millis();
            metrics.status_updates_processed = updates_processed;

            // Process deferred job completions from triggerer
            let mut job_completions = Vec::new();
            while let Ok(completion) = job_completion_rx.try_recv() {
                job_completions.push(completion);
            }
            if !job_completions.is_empty() {
                self.process_job_completions(job_completions).await;
            }

            // Enforce timeouts at most once every 5 seconds to avoid per-loop
            // `get_running_tasks` DB calls when the scheduler is fully loaded.
            let should_check_timeouts = {
                let mut last = self.last_timeout_check.lock().unwrap();
                let now = Instant::now();
                let due = last.is_none_or(|t| now.duration_since(t) >= StdDuration::from_secs(5));
                if due {
                    *last = Some(now);
                }
                due
            };
            if should_check_timeouts && let Err(e) = self.enforce_timeouts().await {
                error!("Error enforcing timeouts: {}", e);
            }

            // Only sleep if there was no work this iteration
            // When work is processed, immediately loop back to check for more
            let had_work = runs_processed > 0 || tasks_processed > 0 || updates_processed > 0;
            let start = Instant::now();

            if !had_work {
                // No work - sleep until status update or timeout
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs_f64(self.config.poll_interval_secs)) => {
                        // Timeout - continue to next iteration
                    }
                    Some(update) = status_rx.recv() => {
                        // Got status update - process it and loop back immediately
                        if let Err(e) = self.process_status_updates(vec![update]).await {
                            error!("Error processing status update: {}", e);
                        }
                    }
                }
            }

            metrics.sleep_ms = start.elapsed().as_millis();
            metrics.total_loop_ms = loop_start.elapsed().as_millis();
            metrics.timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Output structured JSON metrics for easy parsing
            let metrics_json = serde_json::to_string(&metrics).unwrap_or_default();
            debug!("SCHEDULER_METRICS: {}", metrics_json);
        }
    }
}
