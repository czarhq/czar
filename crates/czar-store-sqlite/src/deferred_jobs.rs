use anyhow::Result;
use czar_core::models::{DeferredJob, DeferredJobStatus};
use sqlx::types::Json;
use uuid::Uuid;

use super::SqliteDatabase;

impl SqliteDatabase {
    pub async fn create_deferred_job(
        &self,
        task_id: Uuid,
        service_type: &str,
        job_id: &str,
        job_data: serde_json::Value,
    ) -> Result<DeferredJob> {
        let job = sqlx::query_as::<_, DeferredJob>(
            r#"
            INSERT INTO deferred_jobs (task_id, service_type, job_id, job_data, status)
            VALUES (?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(task_id)
        .bind(service_type)
        .bind(job_id)
        .bind(Json(job_data))
        .bind(DeferredJobStatus::Pending.as_str())
        .fetch_one(&self.pool)
        .await?;

        Ok(job)
    }

    pub async fn get_pending_deferred_jobs(&self) -> Result<Vec<DeferredJob>> {
        let jobs = sqlx::query_as::<_, DeferredJob>(
            r#"
            SELECT * FROM deferred_jobs
            WHERE status IN (?, ?)
            ORDER BY created_at ASC
            "#,
        )
        .bind(DeferredJobStatus::Pending.as_str())
        .bind(DeferredJobStatus::Polling.as_str())
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    pub async fn get_deferred_jobs_for_task(&self, task_id: Uuid) -> Result<Vec<DeferredJob>> {
        let jobs = sqlx::query_as::<_, DeferredJob>(
            r#"
            SELECT * FROM deferred_jobs
            WHERE task_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    pub async fn update_deferred_job_status(
        &self,
        job_id: Uuid,
        status: DeferredJobStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let status_str = status.as_str();
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET status = ?,
                error = ?,
                started_at = CASE WHEN started_at IS NULL AND ? = ? THEN CURRENT_TIMESTAMP ELSE started_at END
            WHERE id = ?
            "#,
        )
        .bind(status_str)
        .bind(error)
        .bind(status_str)
        .bind(DeferredJobStatus::Polling.as_str())
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_deferred_job_polled(&self, job_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET last_polled_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn complete_deferred_job(&self, job_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET status = ?,
                finished_at = CURRENT_TIMESTAMP,
                started_at = CASE WHEN started_at IS NULL THEN CURRENT_TIMESTAMP ELSE started_at END
            WHERE id = ?
            "#,
        )
        .bind(DeferredJobStatus::Completed.as_str())
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn fail_deferred_job(&self, job_id: Uuid, error: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET status = ?,
                error = ?,
                finished_at = CURRENT_TIMESTAMP,
                started_at = CASE WHEN started_at IS NULL THEN CURRENT_TIMESTAMP ELSE started_at END
            WHERE id = ?
            "#,
        )
        .bind(DeferredJobStatus::Failed.as_str())
        .bind(error)
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn cancel_deferred_jobs_for_task(&self, task_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET status = ?,
                finished_at = CURRENT_TIMESTAMP
            WHERE task_id = ?
              AND status IN (?, ?)
            "#,
        )
        .bind(DeferredJobStatus::Cancelled.as_str())
        .bind(task_id)
        .bind(DeferredJobStatus::Pending.as_str())
        .bind(DeferredJobStatus::Polling.as_str())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
