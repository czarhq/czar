//! Dataproc job tracker for Google Cloud Platform

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use czar_core::job_tracker::{JobStatus, JobTracker};

const DATAPROC_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const DEFAULT_DATAPROC_API_BASE: &str = "https://dataproc.googleapis.com/v1";

/// Dataproc job tracker
///
/// Polls Dataproc API to check Spark/Hadoop job status.
pub struct DataprocTracker {
    client: reqwest::Client,
    #[cfg(test)]
    allow_init: bool,
}

impl Default for DataprocTracker {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            #[cfg(test)]
            allow_init: false,
        }
    }
}

impl DataprocTracker {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            #[cfg(test)]
            allow_init: true,
        })
    }

    #[cfg(test)]
    pub(crate) fn without_client() -> Self {
        Self {
            client: reqwest::Client::new(),
            allow_init: false,
        }
    }

    async fn access_token(&self) -> anyhow::Result<String> {
        #[cfg(test)]
        if !self.allow_init {
            anyhow::bail!("Dataproc client not initialized");
        }

        if let Ok(token) = std::env::var("CZAR_DATAPROC_ACCESS_TOKEN")
            && !token.trim().is_empty()
        {
            return Ok(token);
        }

        let provider = gcp_auth::provider().await.map_err(|e| {
            anyhow::anyhow!(
                "Failed to get GCP auth provider for Dataproc. Set GOOGLE_APPLICATION_CREDENTIALS, use workload identity, or set CZAR_DATAPROC_ACCESS_TOKEN: {}",
                e
            )
        })?;
        let token = provider
            .token(&[DATAPROC_SCOPE])
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get Dataproc access token: {}", e))?;
        Ok(token.as_str().to_string())
    }

    fn job_url(&self, project: &str, region: &str, job_id: &str) -> anyhow::Result<Url> {
        let base_url = std::env::var("CZAR_DATAPROC_API_BASE")
            .unwrap_or_else(|_| DEFAULT_DATAPROC_API_BASE.to_string());
        let mut url = Url::parse(base_url.trim_end_matches('/'))
            .map_err(|e| anyhow::anyhow!("Invalid Dataproc API base URL: {}", e))?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Dataproc API base URL cannot be a base"))?
            .extend(["projects", project, "regions", region, "jobs", job_id]);
        Ok(url)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct DataprocJobResponse {
    #[serde(default)]
    status: Option<DataprocJobStatus>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DataprocJobStatus {
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    details: String,
}

pub(crate) fn map_dataproc_job(job: &DataprocJobResponse) -> JobStatus {
    let Some(status) = job.status.as_ref() else {
        return JobStatus::Running;
    };

    match status.state.as_deref() {
        Some("DONE") => {
            if !status.details.is_empty() && status.details.to_lowercase().contains("error") {
                JobStatus::Failed(format!(
                    "Dataproc job completed with errors: {}",
                    status.details
                ))
            } else {
                JobStatus::Completed
            }
        }
        Some("ERROR") => {
            let error_msg = if status.details.is_empty() {
                "Job error".to_string()
            } else {
                status.details.clone()
            };
            JobStatus::Failed(format!("Dataproc job error: {}", error_msg))
        }
        Some("CANCELLED") => JobStatus::Failed("Job was cancelled".to_string()),
        Some("PENDING" | "SETUP_DONE" | "RUNNING" | "CANCEL_PENDING" | "CANCEL_STARTED") | None => {
            JobStatus::Running
        }
        Some(_) => JobStatus::Running,
    }
}

#[async_trait]
impl JobTracker for DataprocTracker {
    fn service_type(&self) -> &str {
        "dataproc"
    }

    async fn poll_job(&self, _job_id: &str, job_data: &Value) -> anyhow::Result<JobStatus> {
        let project = job_data
            .get("project")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing project in job_data"))?;
        let region = job_data
            .get("region")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing region in job_data"))?;
        let _cluster_name = job_data
            .get("cluster_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing cluster_name in job_data"))?;
        let dp_job_id = job_data
            .get("job_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing job_id in job_data"))?;

        tracing::debug!(
            "Polling Dataproc job: project={}, region={}, job_id={}",
            project,
            region,
            dp_job_id
        );

        let token = self.access_token().await?;
        let url = self.job_url(project, region, dp_job_id)?;

        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get Dataproc job: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Failed to get Dataproc job: HTTP {}: {}",
                status,
                error_text
            );
        }

        let job = response
            .json::<DataprocJobResponse>()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to decode Dataproc job response: {}", e))?;

        Ok(map_dataproc_job(&job))
    }
}
