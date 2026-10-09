use serde::Deserialize;

use crate::notifications::NotificationsConfig;
use crate::worker_registry::WorkerConfig;

/// Orchestrator configuration for performance tuning
#[derive(Debug, Clone, Deserialize)]
pub struct OrchestratorConfig {
    /// Polling interval in seconds (supports fractional seconds like 0.5)
    pub poll_interval_secs: f64,

    /// Maximum number of tasks to process per batch
    pub max_tasks_per_batch: i64,

    /// Maximum concurrent task dispatches
    pub max_concurrent_dispatches: usize,

    /// Maximum concurrent status checks
    pub max_concurrent_status_checks: usize,

    /// Database connection pool size
    pub db_pool_size: u32,

    /// Enable deferred-job triggerer startup.
    pub enable_triggerer: bool,

    /// Built-in scheduler notifications.
    #[serde(default)]
    pub notifications: NotificationsConfig,

    /// Remote workers to sync workflows from on startup (and optionally poll).
    #[serde(default)]
    pub workers: Vec<WorkerConfig>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: std::env::var("POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),
            max_tasks_per_batch: std::env::var("MAX_TASKS_PER_BATCH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            max_concurrent_dispatches: std::env::var("MAX_CONCURRENT_DISPATCHES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            max_concurrent_status_checks: std::env::var("MAX_CONCURRENT_STATUS_CHECKS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
            db_pool_size: std::env::var("DB_POOL_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            enable_triggerer: std::env::var("ENABLE_TRIGGERER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            notifications: NotificationsConfig::default(),
            workers: Vec::new(),
        }
    }
}

impl OrchestratorConfig {
    /// Named preset constructors below are convenience starting points.
    /// Users can override any field by writing a YAML config file — all fields
    /// in this struct map directly to YAML keys via `serde::Deserialize`.
    /// The presets exist so callers don't have to spell out every field in
    /// code; they are not special in any way.
    ///
    /// Fast config for in-process tests: 10ms polling, triggerer disabled.
    pub fn fast_test() -> Self {
        Self {
            poll_interval_secs: 0.01,
            enable_triggerer: false,
            ..Self::default()
        }
    }

    /// Create a low-resource configuration: slower polling, smaller batches,
    /// fewer concurrent dispatches, and a smaller DB pool.  Suitable for
    /// resource-constrained environments where throughput is not the priority.
    pub fn low_resource() -> Self {
        Self {
            poll_interval_secs: 2.0,
            max_tasks_per_batch: 50,
            max_concurrent_dispatches: 5,
            max_concurrent_status_checks: 20,
            db_pool_size: 5,
            enable_triggerer: true,
            ..Self::default()
        }
    }

    /// Create a high-throughput configuration
    pub fn high_throughput() -> Self {
        Self {
            poll_interval_secs: 5.0,
            max_tasks_per_batch: 500,
            max_concurrent_dispatches: 50,
            max_concurrent_status_checks: 100,
            db_pool_size: 20,
            enable_triggerer: true,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests;
