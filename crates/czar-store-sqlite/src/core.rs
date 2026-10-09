use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::PathBuf;
use uuid::Uuid;

use czar_core::models::{Task, TaskStatus, TaskWithWorkflow, WorkflowSnapshot, WorkflowTask};

pub(super) fn parse_depends_on(raw: Option<String>) -> Vec<String> {
    match raw {
        Some(value) => serde_json::from_str(&value).unwrap_or_default(),
        None => Vec::new(),
    }
}

pub(super) fn encode_depends_on(deps: &[String]) -> String {
    serde_json::to_string(deps).unwrap_or_else(|_| "[]".to_string())
}

pub(super) fn parse_retry_at(raw: Option<String>) -> Option<DateTime<Utc>> {
    raw.and_then(|value| {
        DateTime::parse_from_rfc3339(&value)
            .map(|dt| dt.with_timezone(&Utc))
            .ok()
    })
}

#[derive(Default, sqlx::FromRow)]
pub(super) struct TaskRow {
    pub id: Uuid,
    pub run_id: Uuid,
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: String,
    pub status: TaskStatus,
    pub attempts: i32,
    pub max_retries: i32,
    pub timeout_seconds: Option<i32>,
    pub retry_at: Option<String>,
    pub execution_name: Option<String>,
    pub params: Option<sqlx::types::Json<serde_json::Value>>,
    pub output: Option<sqlx::types::Json<serde_json::Value>>,
    pub logs: Option<String>,
    pub error: Option<String>,
    pub progress: Option<sqlx::types::Json<serde_json::Value>>,
    pub dispatched_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Default, sqlx::FromRow)]
pub(super) struct WorkflowTaskRow {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: String,
    pub params: Option<sqlx::types::Json<serde_json::Value>>,
    pub signature: Option<sqlx::types::Json<serde_json::Value>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Default, sqlx::FromRow)]
pub(super) struct TaskWithWorkflowRow {
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: String,
    pub task_status: TaskStatus,
    pub attempts: i32,
    pub max_retries: i32,
    pub timeout_seconds: Option<i32>,
    pub retry_at: Option<String>,
    pub execution_name: Option<String>,
    pub params: Option<sqlx::types::Json<serde_json::Value>>,
    pub workflow_id: Uuid,
    pub job_name: String,
    pub project: String,
    pub region: String,
}

#[derive(Default, sqlx::FromRow)]
pub(super) struct WorkflowSnapshotRow {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub content_hash: String,
    pub tasks_json: String,
    pub source_yaml: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct SqliteDatabase {
    pub(super) pool: SqlitePool,
}

impl Default for SqliteDatabase {
    fn default() -> Self {
        let pool = SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .expect("default sqlite pool should build");
        Self { pool }
    }
}

fn sqlite_database_file_path(database_url: &str) -> Option<PathBuf> {
    let raw = database_url
        .strip_prefix("sqlite://")
        .or_else(|| database_url.strip_prefix("sqlite:"))?;

    let path = raw.split('?').next().unwrap_or(raw);
    if path.is_empty() || path == ":memory:" || path.starts_with("file:") {
        return None;
    }

    Some(PathBuf::from(path))
}

impl SqliteDatabase {
    pub async fn new(database_url: &str) -> Result<Self> {
        if let Some(path) = sqlite_database_file_path(database_url)
            && let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create SQLite database directory: {}",
                    parent.display()
                )
            })?;
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(database_url)
            .await?;

        // Enable foreign keys
        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await?;

        // Enable WAL mode for better concurrency (allows concurrent reads during writes)
        sqlx::query("PRAGMA journal_mode = WAL;")
            .execute(&pool)
            .await?;

        // Reduce fsync overhead - NORMAL is safe and much faster than FULL
        sqlx::query("PRAGMA synchronous = NORMAL;")
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations_sqlite")
            .run(&self.pool)
            .await?;
        Ok(())
    }

    pub(super) fn map_task(row: TaskRow) -> Task {
        Task {
            id: row.id,
            run_id: row.run_id,
            task_index: row.task_index,
            task_name: row.task_name,
            executor_type: row.executor_type,
            depends_on: parse_depends_on(Some(row.depends_on)),
            status: row.status,
            attempts: row.attempts,
            max_retries: row.max_retries,
            timeout_seconds: row.timeout_seconds,
            retry_at: parse_retry_at(row.retry_at),
            execution_name: row.execution_name,
            params: row.params,
            output: row.output,
            logs: row.logs,
            error: row.error,
            progress: row.progress,
            dispatched_at: row.dispatched_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
            created_at: row.created_at,
        }
    }

    pub(super) fn map_workflow_task(row: WorkflowTaskRow) -> WorkflowTask {
        WorkflowTask {
            id: row.id,
            workflow_id: row.workflow_id,
            task_index: row.task_index,
            task_name: row.task_name,
            executor_type: row.executor_type,
            depends_on: parse_depends_on(Some(row.depends_on)),
            params: row.params,
            signature: row.signature,
            created_at: row.created_at,
        }
    }

    pub(super) fn map_task_with_workflow(row: TaskWithWorkflowRow) -> TaskWithWorkflow {
        TaskWithWorkflow {
            task_id: row.task_id,
            run_id: row.run_id,
            task_index: row.task_index,
            task_name: row.task_name,
            executor_type: row.executor_type,
            depends_on: parse_depends_on(Some(row.depends_on)),
            task_status: row.task_status,
            attempts: row.attempts,
            max_retries: row.max_retries,
            timeout_seconds: row.timeout_seconds,
            retry_at: parse_retry_at(row.retry_at),
            execution_name: row.execution_name,
            params: row.params,
            workflow_id: row.workflow_id,
            job_name: row.job_name,
            project: row.project,
            region: row.region,
        }
    }

    pub(super) fn map_workflow_snapshot(row: WorkflowSnapshotRow) -> Result<WorkflowSnapshot> {
        let tasks_json = serde_json::from_str(&row.tasks_json)?;
        Ok(WorkflowSnapshot {
            id: row.id,
            workflow_id: row.workflow_id,
            content_hash: row.content_hash,
            tasks_json: sqlx::types::Json(tasks_json),
            source_yaml: row.source_yaml,
            created_at: row.created_at,
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(test)]
mod tests;
