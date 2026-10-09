use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use czar_core::database::{Database as CzarDatabase, DeferredJobRepository, ScheduleRepository};
use czar_core::models::{DeferredJob, DeferredJobStatus, Workflow};

use crate::client::QueryParam;
use crate::database::{D1Database, decode, opt_datetime, opt_text, uuid_param};

impl D1Database {
    pub(crate) async fn update_workflow_schedule_times_impl(
        &self,
        workflow_id: Uuid,
        last_scheduled_at: DateTime<Utc>,
        next_scheduled_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        self.execute(
            "UPDATE workflows SET last_scheduled_at = ?, next_scheduled_at = ?, updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            vec![
                crate::database::datetime_param(last_scheduled_at),
                opt_datetime(next_scheduled_at),
                uuid_param(workflow_id),
            ],
        )
        .await
    }

    pub(crate) async fn update_workflow_schedule_impl(
        &self,
        workflow_id: Uuid,
        schedule: Option<&str>,
        schedule_enabled: bool,
    ) -> Result<()> {
        self.execute(
            "UPDATE workflows SET schedule = ?, schedule_enabled = ?, updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            vec![
                opt_text(schedule),
                QueryParam::Integer(i64::from(schedule_enabled as i32)),
                uuid_param(workflow_id),
            ],
        )
        .await
    }

    pub(crate) async fn create_deferred_job_impl(
        &self,
        task_id: Uuid,
        service_type: &str,
        job_id: &str,
        job_data: Value,
    ) -> Result<DeferredJob> {
        self.fetch_one(
            r#"INSERT INTO deferred_jobs (id, task_id, service_type, job_id, job_data, status)
               VALUES (?, ?, ?, ?, ?, ?) RETURNING *"#,
            vec![
                uuid_param(Uuid::new_v4()),
                uuid_param(task_id),
                service_type.into(),
                job_id.into(),
                QueryParam::Text(job_data.to_string()),
                DeferredJobStatus::Pending.as_str().into(),
            ],
            decode::deferred_job,
        )
        .await
    }

    pub(crate) async fn get_pending_deferred_jobs_impl(&self) -> Result<Vec<DeferredJob>> {
        self.fetch_all(
            "SELECT * FROM deferred_jobs WHERE status IN (?, ?) ORDER BY created_at ASC",
            vec![
                DeferredJobStatus::Pending.as_str().into(),
                DeferredJobStatus::Polling.as_str().into(),
            ],
            decode::deferred_job,
        )
        .await
    }

    pub(crate) async fn get_deferred_jobs_for_task_impl(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<DeferredJob>> {
        self.fetch_all(
            "SELECT * FROM deferred_jobs WHERE task_id = ? ORDER BY created_at ASC",
            vec![uuid_param(task_id)],
            decode::deferred_job,
        )
        .await
    }

    pub(crate) async fn update_deferred_job_status_impl(
        &self,
        job_id: Uuid,
        status: DeferredJobStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let status_str = status.as_str();
        self.execute(
            r#"UPDATE deferred_jobs
               SET status = ?,
                   error = ?,
                   started_at = CASE WHEN started_at IS NULL AND ? = ? THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END
               WHERE id = ?"#,
            vec![
                status_str.into(),
                opt_text(error),
                status_str.into(),
                DeferredJobStatus::Polling.as_str().into(),
                uuid_param(job_id),
            ],
        )
        .await
    }

    pub(crate) async fn update_deferred_job_polled_impl(&self, job_id: Uuid) -> Result<()> {
        self.execute(
            "UPDATE deferred_jobs SET last_polled_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            vec![uuid_param(job_id)],
        )
        .await
    }

    pub(crate) async fn complete_deferred_job_impl(&self, job_id: Uuid) -> Result<()> {
        self.execute(
            r#"UPDATE deferred_jobs
               SET status = ?,
                   finished_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now'),
                   started_at = CASE WHEN started_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END
               WHERE id = ?"#,
            vec![
                DeferredJobStatus::Completed.as_str().into(),
                uuid_param(job_id),
            ],
        )
        .await
    }

    pub(crate) async fn fail_deferred_job_impl(&self, job_id: Uuid, error: &str) -> Result<()> {
        self.execute(
            r#"UPDATE deferred_jobs
               SET status = ?,
                   error = ?,
                   finished_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now'),
                   started_at = CASE WHEN started_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END
               WHERE id = ?"#,
            vec![
                DeferredJobStatus::Failed.as_str().into(),
                error.into(),
                uuid_param(job_id),
            ],
        )
        .await
    }

    pub(crate) async fn cancel_deferred_jobs_for_task_impl(&self, task_id: Uuid) -> Result<()> {
        self.execute(
            r#"UPDATE deferred_jobs
               SET status = ?,
                   finished_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')
               WHERE task_id = ?
                 AND status IN (?, ?)"#,
            vec![
                DeferredJobStatus::Cancelled.as_str().into(),
                uuid_param(task_id),
                DeferredJobStatus::Pending.as_str().into(),
                DeferredJobStatus::Polling.as_str().into(),
            ],
        )
        .await
    }
}

#[async_trait]
impl ScheduleRepository for D1Database {
    async fn get_due_scheduled_workflows(&self) -> Result<Vec<Workflow>> {
        self.get_due_scheduled_workflows_impl().await
    }

    async fn update_workflow_schedule_times(
        &self,
        workflow_id: Uuid,
        last_scheduled_at: DateTime<Utc>,
        next_scheduled_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        self.update_workflow_schedule_times_impl(workflow_id, last_scheduled_at, next_scheduled_at)
            .await
    }

    async fn update_workflow_schedule(
        &self,
        workflow_id: Uuid,
        schedule: Option<&str>,
        schedule_enabled: bool,
    ) -> Result<()> {
        self.update_workflow_schedule_impl(workflow_id, schedule, schedule_enabled)
            .await
    }
}

#[async_trait]
impl DeferredJobRepository for D1Database {
    async fn create_deferred_job(
        &self,
        task_id: Uuid,
        service_type: &str,
        job_id: &str,
        job_data: Value,
    ) -> Result<DeferredJob> {
        self.create_deferred_job_impl(task_id, service_type, job_id, job_data)
            .await
    }

    async fn get_pending_deferred_jobs(&self) -> Result<Vec<DeferredJob>> {
        self.get_pending_deferred_jobs_impl().await
    }

    async fn get_deferred_jobs_for_task(&self, task_id: Uuid) -> Result<Vec<DeferredJob>> {
        self.get_deferred_jobs_for_task_impl(task_id).await
    }

    async fn update_deferred_job_status(
        &self,
        job_id: Uuid,
        status: DeferredJobStatus,
        error: Option<&str>,
    ) -> Result<()> {
        self.update_deferred_job_status_impl(job_id, status, error)
            .await
    }

    async fn update_deferred_job_polled(&self, job_id: Uuid) -> Result<()> {
        self.update_deferred_job_polled_impl(job_id).await
    }

    async fn complete_deferred_job(&self, job_id: Uuid) -> Result<()> {
        self.complete_deferred_job_impl(job_id).await
    }

    async fn fail_deferred_job(&self, job_id: Uuid, error: &str) -> Result<()> {
        self.fail_deferred_job_impl(job_id, error).await
    }

    async fn cancel_deferred_jobs_for_task(&self, task_id: Uuid) -> Result<()> {
        self.cancel_deferred_jobs_for_task_impl(task_id).await
    }
}

#[async_trait]
impl CzarDatabase for D1Database {
    async fn run_migrations(&self) -> Result<()> {
        D1Database::run_migrations(self).await
    }
}
