use anyhow::Result;
use czar_core::models::{DeferredJob, DeferredJobStatus};
use std::path::PathBuf;
use uuid::Uuid;

use super::FileDatabase;

impl FileDatabase {
    fn deferred_jobs_dir(&self) -> PathBuf {
        self.base.join("deferred_jobs")
    }

    fn deferred_job_path(&self, job_id: Uuid) -> PathBuf {
        self.deferred_jobs_dir().join(format!("{}.json", job_id))
    }

    pub async fn create_deferred_job(
        &self,
        task_id: Uuid,
        service_type: &str,
        job_id: &str,
        job_data: serde_json::Value,
    ) -> Result<DeferredJob> {
        tokio::fs::create_dir_all(self.deferred_jobs_dir()).await?;

        let now = chrono::Utc::now();
        let job = DeferredJob {
            id: Uuid::new_v4(),
            task_id,
            service_type: service_type.to_string(),
            job_id: job_id.to_string(),
            job_data: job_data.into(),
            status: DeferredJobStatus::Pending,
            error: None,
            created_at: now,
            started_at: None,
            last_polled_at: None,
            finished_at: None,
        };

        let json = serde_json::to_string_pretty(&job)?;
        tokio::fs::write(self.deferred_job_path(job.id), json).await?;

        Ok(job)
    }

    pub async fn get_pending_deferred_jobs(&self) -> Result<Vec<DeferredJob>> {
        let dir = self.deferred_jobs_dir();
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut jobs = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(content) = tokio::fs::read_to_string(entry.path()).await
                && let Ok(job) = serde_json::from_str::<DeferredJob>(&content)
            {
                let status = job.status();
                if status == DeferredJobStatus::Pending || status == DeferredJobStatus::Polling {
                    jobs.push(job);
                }
            }
        }

        // Sort by created_at
        jobs.sort_by_key(|job| job.created_at);
        Ok(jobs)
    }

    pub async fn get_deferred_jobs_for_task(&self, task_id: Uuid) -> Result<Vec<DeferredJob>> {
        let dir = self.deferred_jobs_dir();
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut jobs = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(content) = tokio::fs::read_to_string(entry.path()).await
                && let Ok(job) = serde_json::from_str::<DeferredJob>(&content)
                && job.task_id == task_id
            {
                jobs.push(job);
            }
        }

        // Sort by created_at
        jobs.sort_by_key(|job| job.created_at);
        Ok(jobs)
    }

    pub async fn update_deferred_job_status(
        &self,
        job_id: Uuid,
        status: DeferredJobStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let path = self.deferred_job_path(job_id);
        if !path.exists() {
            return Err(anyhow::anyhow!("Deferred job not found: {}", job_id));
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let mut job: DeferredJob = serde_json::from_str(&content)?;

        job.status = status;
        job.error = error.map(|s| s.to_string());

        // Set started_at if transitioning to polling
        if matches!(status, DeferredJobStatus::Polling) && job.started_at.is_none() {
            job.started_at = Some(chrono::Utc::now());
        }

        let json = serde_json::to_string_pretty(&job)?;
        tokio::fs::write(&path, json).await?;

        Ok(())
    }

    pub async fn update_deferred_job_polled(&self, job_id: Uuid) -> Result<()> {
        let path = self.deferred_job_path(job_id);
        if !path.exists() {
            return Err(anyhow::anyhow!("Deferred job not found: {}", job_id));
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let mut job: DeferredJob = serde_json::from_str(&content)?;

        job.last_polled_at = Some(chrono::Utc::now());

        let json = serde_json::to_string_pretty(&job)?;
        tokio::fs::write(&path, json).await?;

        Ok(())
    }

    pub async fn complete_deferred_job(&self, job_id: Uuid) -> Result<()> {
        let path = self.deferred_job_path(job_id);
        if !path.exists() {
            return Err(anyhow::anyhow!("Deferred job not found: {}", job_id));
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let mut job: DeferredJob = serde_json::from_str(&content)?;

        job.status = DeferredJobStatus::Completed;
        job.finished_at = Some(chrono::Utc::now());

        // Set started_at if not already set
        if job.started_at.is_none() {
            job.started_at = Some(chrono::Utc::now());
        }

        let json = serde_json::to_string_pretty(&job)?;
        tokio::fs::write(&path, json).await?;

        Ok(())
    }

    pub async fn fail_deferred_job(&self, job_id: Uuid, error: &str) -> Result<()> {
        let path = self.deferred_job_path(job_id);
        if !path.exists() {
            return Err(anyhow::anyhow!("Deferred job not found: {}", job_id));
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let mut job: DeferredJob = serde_json::from_str(&content)?;

        job.status = DeferredJobStatus::Failed;
        job.error = Some(error.to_string());
        job.finished_at = Some(chrono::Utc::now());

        // Set started_at if not already set
        if job.started_at.is_none() {
            job.started_at = Some(chrono::Utc::now());
        }

        let json = serde_json::to_string_pretty(&job)?;
        tokio::fs::write(&path, json).await?;

        Ok(())
    }

    pub async fn cancel_deferred_jobs_for_task(&self, task_id: Uuid) -> Result<()> {
        let dir = self.deferred_jobs_dir();
        if !dir.exists() {
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(&dir).await?;
        let now = chrono::Utc::now();

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(content) = tokio::fs::read_to_string(entry.path()).await
                && let Ok(mut job) = serde_json::from_str::<DeferredJob>(&content)
                && job.task_id == task_id
            {
                let status = job.status();
                if status == DeferredJobStatus::Pending || status == DeferredJobStatus::Polling {
                    job.status = DeferredJobStatus::Cancelled;
                    job.finished_at = Some(now);

                    let json = serde_json::to_string_pretty(&job)?;
                    tokio::fs::write(entry.path(), json).await?;
                }
            }
        }

        Ok(())
    }
}
