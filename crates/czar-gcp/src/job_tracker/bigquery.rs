//! BigQuery job tracker for Google Cloud Platform

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use czar_core::job_tracker::{JobStatus, JobTracker};

const BIGQUERY_SCOPE: &str = "https://www.googleapis.com/auth/bigquery";
const DEFAULT_BIGQUERY_API_BASE: &str = "https://bigquery.googleapis.com/bigquery/v2";

/// BigQuery job tracker
///
/// Polls BigQuery API to check query job status.
pub struct BigQueryTracker {
    client: reqwest::Client,
    #[cfg(test)]
    allow_init: bool,
}

impl Default for BigQueryTracker {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            #[cfg(test)]
            allow_init: false,
        }
    }
}

impl BigQueryTracker {
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
            anyhow::bail!("BigQuery client not initialized");
        }

        if let Ok(token) = std::env::var("CZAR_BIGQUERY_ACCESS_TOKEN")
            && !token.trim().is_empty()
        {
            return Ok(token);
        }

        let provider = gcp_auth::provider().await.map_err(|e| {
            anyhow::anyhow!(
                "Failed to get GCP auth provider for BigQuery. Set GOOGLE_APPLICATION_CREDENTIALS, use workload identity, or set CZAR_BIGQUERY_ACCESS_TOKEN: {}",
                e
            )
        })?;
        let token = provider
            .token(&[BIGQUERY_SCOPE])
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get BigQuery access token: {}", e))?;
        Ok(token.as_str().to_string())
    }

    fn job_url(&self, project: &str, job_id: &str, location: &str) -> anyhow::Result<Url> {
        let base_url = std::env::var("CZAR_BIGQUERY_API_BASE")
            .unwrap_or_else(|_| DEFAULT_BIGQUERY_API_BASE.to_string());
        let mut url = Url::parse(base_url.trim_end_matches('/'))
            .map_err(|e| anyhow::anyhow!("Invalid BigQuery API base URL: {}", e))?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("BigQuery API base URL cannot be a base"))?
            .extend(["projects", project, "jobs", job_id]);
        url.query_pairs_mut().append_pair("location", location);
        Ok(url)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct BigQueryJobStatus {
    #[serde(default)]
    state: Option<String>,
    #[serde(default, rename = "errorResult")]
    error_result: Option<BigQueryErrorProto>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BigQueryErrorProto {
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BigQueryJobResponse {
    status: BigQueryJobStatus,
}

pub(crate) fn map_bigquery_status(status: &BigQueryJobStatus) -> JobStatus {
    match status.state.as_deref() {
        Some("DONE") => {
            if let Some(error_result) = status.error_result.as_ref() {
                let error_msg = error_result
                    .message
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string());
                JobStatus::Failed(format!("BigQuery job failed: {}", error_msg))
            } else {
                JobStatus::Completed
            }
        }
        Some("PENDING" | "RUNNING") | None => JobStatus::Running,
        Some(state) => JobStatus::Failed(format!("Unknown BigQuery job state: {}", state)),
    }
}

#[async_trait]
impl JobTracker for BigQueryTracker {
    fn service_type(&self) -> &str {
        "bigquery"
    }

    async fn poll_job(&self, _job_id: &str, job_data: &Value) -> anyhow::Result<JobStatus> {
        // Extract BigQuery job info
        let project = job_data
            .get("project")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing project in job_data"))?;
        let location = job_data
            .get("location")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing location in job_data"))?;
        let bq_job_id = job_data
            .get("job_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing job_id in job_data"))?;

        tracing::debug!(
            "Polling BigQuery job: project={}, location={}, job_id={}",
            project,
            location,
            bq_job_id
        );

        let token = self.access_token().await?;
        let url = self.job_url(project, bq_job_id, location)?;

        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get BigQuery job: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Failed to get BigQuery job: HTTP {}: {}",
                status,
                error_text
            );
        }

        let job = response
            .json::<BigQueryJobResponse>()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to decode BigQuery job response: {}", e))?;

        Ok(map_bigquery_status(&job.status))
    }
}
