//! Cloud Run job tracker for Google Cloud Platform

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use czar_core::job_tracker::{JobStatus, JobTracker};

const CLOUDRUN_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const DEFAULT_CLOUDRUN_API_BASE: &str = "https://run.googleapis.com/v2";

/// Cloud Run job tracker
///
/// Polls Cloud Run API to check job execution status.
pub struct CloudRunTracker {
    client: reqwest::Client,
    #[cfg(test)]
    allow_init: bool,
}

impl Default for CloudRunTracker {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            #[cfg(test)]
            allow_init: false,
        }
    }
}

impl CloudRunTracker {
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
            anyhow::bail!("Cloud Run client not initialized");
        }

        if let Ok(token) = std::env::var("CZAR_CLOUDRUN_ACCESS_TOKEN")
            && !token.trim().is_empty()
        {
            return Ok(token);
        }

        let provider = gcp_auth::provider().await.map_err(|e| {
            anyhow::anyhow!(
                "Failed to get GCP auth provider for Cloud Run. Set GOOGLE_APPLICATION_CREDENTIALS, use workload identity, or set CZAR_CLOUDRUN_ACCESS_TOKEN: {}",
                e
            )
        })?;
        let token = provider
            .token(&[CLOUDRUN_SCOPE])
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get Cloud Run access token: {}", e))?;
        Ok(token.as_str().to_string())
    }

    fn execution_url(
        &self,
        project: &str,
        region: &str,
        job_name: &str,
        execution_id: &str,
    ) -> anyhow::Result<Url> {
        let base_url = std::env::var("CZAR_CLOUDRUN_API_BASE")
            .unwrap_or_else(|_| DEFAULT_CLOUDRUN_API_BASE.to_string());
        let mut url = Url::parse(base_url.trim_end_matches('/'))
            .map_err(|e| anyhow::anyhow!("Invalid Cloud Run API base URL: {}", e))?;

        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| anyhow::anyhow!("Cloud Run API base URL cannot be a base"))?;
            if execution_id.starts_with("projects/") {
                segments.extend(execution_id.split('/'));
            } else {
                segments.extend([
                    "projects",
                    project,
                    "locations",
                    region,
                    "jobs",
                    job_name,
                    "executions",
                    execution_id,
                ]);
            }
        }
        Ok(url)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct CloudRunExecutionResponse {
    #[serde(default)]
    conditions: Vec<CloudRunCondition>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CloudRunCondition {
    #[serde(default, rename = "type")]
    condition_type: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    message: String,
}

pub(crate) fn map_cloudrun_execution(response: &CloudRunExecutionResponse) -> JobStatus {
    for condition in &response.conditions {
        if condition.condition_type == "Ready" || condition.condition_type == "Completed" {
            match condition.state.as_str() {
                "CONDITION_SUCCEEDED" | "ConditionSucceeded" | "True" => {
                    return JobStatus::Completed;
                }
                "CONDITION_FAILED" | "ConditionFailed" | "False" => {
                    let error_msg = if condition.message.is_empty() {
                        "Execution failed".to_string()
                    } else {
                        condition.message.clone()
                    };
                    return JobStatus::Failed(format!("Cloud Run execution failed: {}", error_msg));
                }
                _ => {}
            }
        }
    }

    JobStatus::Running
}

#[async_trait]
impl JobTracker for CloudRunTracker {
    fn service_type(&self) -> &str {
        "cloudrun"
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
        let job_name = job_data
            .get("job_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing job_name in job_data"))?;
        let execution_id = job_data
            .get("execution_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing execution_id in job_data"))?;

        tracing::debug!(
            "Polling Cloud Run job: project={}, region={}, job={}, execution={}",
            project,
            region,
            job_name,
            execution_id
        );

        let token = self.access_token().await?;
        let url = self.execution_url(project, region, job_name, execution_id)?;

        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get Cloud Run execution: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Failed to get Cloud Run execution: HTTP {}: {}",
                status,
                error_text
            );
        }

        let execution = response
            .json::<CloudRunExecutionResponse>()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to decode Cloud Run execution: {}", e))?;

        Ok(map_cloudrun_execution(&execution))
    }
}
