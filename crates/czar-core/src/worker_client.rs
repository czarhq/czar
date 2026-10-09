//! Worker client for calling worker HTTP endpoints
//!
//! The scheduler uses this client to:
//! - Compile workflows via POST /compile
//! - Execute tasks via POST /execute

use crate::models::ExecutorType;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Worker client for HTTP communication with worker containers
pub struct WorkerClient {
    client: reqwest::Client,
    worker_url: String,
    token: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct CompileRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CompileResponse {
    pub workflow: serde_json::Value,
    pub tasks: Vec<TaskDefinition>,
    pub source_yaml: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TaskDefinition {
    pub name: String,
    pub executor: String,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ExecuteRequest {
    pub task_id: Uuid,
    pub task_name: String,
    pub executor_type: ExecutorType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl Default for ExecuteRequest {
    fn default() -> Self {
        Self {
            task_id: Uuid::nil(),
            task_name: "task".to_string(),
            executor_type: ExecutorType::Process,
            params: None,
        }
    }
}

impl Default for WorkerClient {
    fn default() -> Self {
        Self::new("http://127.0.0.1:8081".to_string())
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status")]
pub enum ExecuteResponse {
    #[serde(rename = "success")]
    Success {
        output: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        deferred: Option<Vec<serde_json::Value>>,
    },
    #[serde(rename = "failed")]
    Failed { error: String },
}

impl WorkerClient {
    /// Create a new worker client
    ///
    /// # Arguments
    /// * `worker_url` - Base URL of the worker server (e.g., "http://localhost:8081")
    pub fn new(worker_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute timeout for task execution
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            worker_url,
            token: None,
        }
    }

    /// Send `Authorization: Bearer <token>` with every request.
    pub fn with_token(mut self, token: Option<String>) -> Self {
        self.token = token;
        self
    }

    /// Compile a workflow by calling POST /compile
    pub async fn compile(&self, workflow_path: Option<String>) -> Result<CompileResponse> {
        let url = format!("{}/compile", self.worker_url);
        let req = CompileRequest { workflow_path };

        let response = self.post(&url).json(&req).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Compile request failed with status {}: {}",
                status,
                error_text
            ));
        }

        let compile_response: CompileResponse = response.json().await?;
        Ok(compile_response)
    }

    /// Execute a task by calling POST /execute
    pub async fn execute(&self, req: ExecuteRequest) -> Result<ExecuteResponse> {
        let url = format!("{}/execute", self.worker_url);

        tracing::debug!(
            "Calling worker execute: task={}, executor={}",
            req.task_name,
            req.executor_type
        );

        let response = self.post(&url).json(&req).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Execute request failed with status {}: {}",
                status,
                error_text
            ));
        }

        let execute_response: ExecuteResponse = response.json().await?;
        Ok(execute_response)
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        let url = format!("{}/health", self.worker_url);

        let response = self.post(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Health check failed with status {}",
                response.status()
            ));
        }

        Ok(())
    }
}

// Helpers

impl WorkerClient {
    fn post(&self, url: &str) -> reqwest::RequestBuilder {
        let request = self.client.post(url);
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }
}

#[cfg(test)]
mod tests;
