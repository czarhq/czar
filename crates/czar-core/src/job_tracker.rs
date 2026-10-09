//! Job tracking for deferrables
//!
//! This module defines the JobTracker trait and implementations for polling
//! external APIs to track long-running jobs.
//!
//! ## Available Trackers
//!
//! - `CustomHttpTracker` - Always available, polls custom HTTP endpoints
//! - Cloud provider trackers live in provider crates such as `czar-gcp`

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// Status of an external job being tracked
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// Job is still running
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed
    Failed(String),
}

/// Trait for tracking external job status
///
/// Implementations poll external APIs to check job status.
/// Used by the Triggerer component.
#[async_trait]
pub trait JobTracker: Send + Sync {
    /// Get the service type this tracker handles
    fn service_type(&self) -> &str;

    /// Poll the external API to check job status
    ///
    /// # Arguments
    /// * `job_id` - External job identifier
    /// * `job_data` - Full deferrable JSON data (project, location, etc.)
    ///
    /// # Returns
    /// Current job status
    async fn poll_job(&self, job_id: &str, job_data: &Value) -> anyhow::Result<JobStatus>;
}

/// Custom HTTP job tracker
///
/// Polls a custom HTTP endpoint to check job status.
pub struct CustomHttpTracker {
    client: reqwest::Client,
}

impl CustomHttpTracker {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for CustomHttpTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl JobTracker for CustomHttpTracker {
    fn service_type(&self) -> &str {
        "custom_http"
    }

    async fn poll_job(&self, _job_id: &str, job_data: &Value) -> anyhow::Result<JobStatus> {
        let url = job_data
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing url in job_data"))?;

        if let Some(mock_state) = url.strip_prefix("mock://") {
            return match mock_state {
                state if state.starts_with("success") => Ok(JobStatus::Completed),
                state if state.starts_with("failed") => {
                    Ok(JobStatus::Failed("Mock job failed".to_string()))
                }
                _ => Ok(JobStatus::Running),
            };
        }

        let method = job_data
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET");

        let headers = job_data
            .get("headers")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect::<HashMap<String, String>>()
            })
            .unwrap_or_default();

        tracing::debug!("Polling custom HTTP endpoint: {} {}", method, url);

        // Build request
        let mut request = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported HTTP method: {}. Use GET, POST, PUT, or DELETE.",
                    method
                ));
            }
        };

        // Add headers
        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("HTTP request failed: {}", e))?;

        // Get response body as JSON
        let status_code = response.status();
        let response_body: Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse response as JSON: {}", e))?;

        // Check if request failed
        if !status_code.is_success() {
            return Ok(JobStatus::Failed(format!(
                "HTTP request failed with status {}: {}",
                status_code,
                response_body
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
            )));
        }

        // Extract status field from response
        let status_field = job_data
            .get("status_field")
            .and_then(|v| v.as_str())
            .unwrap_or("status");

        let field_value = response_body
            .get(status_field)
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Response missing expected field '{}'. Response: {}",
                    status_field,
                    response_body
                )
            })?;

        // Check if job completed successfully
        let success_value = job_data
            .get("success_value")
            .and_then(|v| v.as_str())
            .unwrap_or("success");

        if field_value == success_value {
            return Ok(JobStatus::Completed);
        }

        // Check if job failed
        let failure_value = job_data.get("failure_value").and_then(|v| v.as_str());

        if let Some(fail_val) = failure_value
            && field_value == fail_val
        {
            return Ok(JobStatus::Failed(format!(
                "Job failed with status: {}",
                field_value
            )));
        }

        // Job is still running
        Ok(JobStatus::Running)
    }
}

#[cfg(test)]
mod tests;
