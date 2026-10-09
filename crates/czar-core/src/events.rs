// Platform-agnostic event types shared between native and WASM builds.

use uuid::Uuid;

/// A status update pushed from an executor back to the scheduler.
///
/// Executors send these via a channel as tasks progress through their
/// lifecycle.  The scheduler processes them in batches and writes the
/// results to the database.
#[derive(Debug, Clone, Default)]
pub struct StatusUpdate {
    pub task_id: Uuid,
    /// One of: "running", "success", "failed", "progress"
    pub status: String,
    /// Incremental log chunk to append to the task log.
    pub log: Option<String>,
    /// Final JSON output produced by the task.
    pub output: Option<serde_json::Value>,
    /// Error message when status is "failed".
    pub error: Option<String>,
    /// Structured progress snapshot when status is "progress".
    /// Shape: `{"current": u64, "total": u64, "message": str}`.
    pub progress: Option<serde_json::Value>,
}
