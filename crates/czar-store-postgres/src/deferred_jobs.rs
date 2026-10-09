use anyhow::Result;
use czar_core::models::{DeferredJob, DeferredJobStatus};
use sqlx::types::Json;
use uuid::Uuid;

use super::PostgresDatabase;

impl PostgresDatabase {
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
            VALUES ($1, $2, $3, $4, $5)
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
            WHERE status IN ($1, $2)
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
            WHERE task_id = $1
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
            SET status = $1,
                error = $2,
                started_at = CASE WHEN started_at IS NULL AND $1 = $3 THEN NOW() ELSE started_at END
            WHERE id = $4
            "#,
        )
        .bind(status_str)
        .bind(error)
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
            SET last_polled_at = NOW()
            WHERE id = $1
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
            SET status = $1,
                finished_at = NOW(),
                started_at = CASE WHEN started_at IS NULL THEN NOW() ELSE started_at END
            WHERE id = $2
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
            SET status = $1,
                error = $2,
                finished_at = NOW(),
                started_at = CASE WHEN started_at IS NULL THEN NOW() ELSE started_at END
            WHERE id = $3
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
            SET status = $1,
                finished_at = NOW()
            WHERE task_id = $2
              AND status IN ($3, $4)
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
