#![allow(dead_code)] // API structs keep all fields for completeness

use anyhow::{Context, Result};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use url::Url;

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Default, Deserialize, Clone, Debug)]
pub struct Workflow {
    pub name: String,
    pub schedule: Option<String>,
    pub schedule_enabled: bool,
    pub next_scheduled_at: Option<String>,
}

#[derive(Default, Deserialize, Debug)]
pub struct WorkflowsResponse {
    pub items: Vec<Workflow>,
    pub total: usize,
}

#[derive(Default, Deserialize, Clone, Debug)]
pub struct Run {
    pub id: String,
    pub workflow: String,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Default, Deserialize, Debug)]
pub struct RunsResponse {
    pub items: Vec<Run>,
    pub total: usize,
}

#[derive(Default, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub status: String,
    pub executor: String,
    pub depends_on: Vec<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub dispatched_at: Option<String>,
    pub output: Option<serde_json::Value>,
    pub logs: Option<String>,
    pub error: Option<String>,
    pub progress: Option<serde_json::Value>,
}

#[derive(Default, Deserialize, Clone, Debug)]
pub struct WorkflowTaskInfo {
    pub name: String,
    pub depends_on: Vec<String>,
    pub executor: String,
}

#[derive(Default, Deserialize, Clone, Debug)]
pub struct WorkflowInfo {
    pub name: String,
    pub tasks: Vec<WorkflowTaskInfo>,
    pub flows: Vec<String>,
    pub schedule: Option<String>,
    pub schedule_enabled: bool,
    pub next_scheduled_at: Option<String>,
}

#[derive(Default, Deserialize, Clone, Debug)]
pub struct RunDetail {
    pub run: Run,
    pub tasks: Vec<Task>,
    pub workflow: Option<WorkflowInfo>,
}

#[derive(Default, Deserialize, Debug)]
pub struct CreateRunResponse {
    pub run_id: String,
}

// ── API client ────────────────────────────────────────────────────────────────

pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new("http://127.0.0.1:4000")
    }
}

impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            token: None,
        }
    }

    /// Send `Authorization: Bearer <token>` (the server's `--auth-token`) with every request.
    pub fn with_token(mut self, token: Option<String>) -> Self {
        self.token = token.filter(|token| !token.trim().is_empty());
        self
    }

    pub fn is_local(&self) -> bool {
        let u = &self.base_url;
        u.contains("127.0.0.1") || u.contains("localhost") || u.contains("::1")
    }

    pub async fn fetch_workflows(
        &self,
        limit: usize,
        offset: usize,
        search: Option<&str>,
    ) -> Result<WorkflowsResponse> {
        let mut url = self.url("/api/workflows")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("limit", &limit.to_string());
            q.append_pair("offset", &offset.to_string());
            if let Some(s) = search {
                q.append_pair("search", s);
            }
        }
        Ok(self.send(Method::GET, url, None).await?.json().await?)
    }

    pub async fn fetch_runs(
        &self,
        limit: usize,
        offset: usize,
        workflow: Option<&str>,
        status: Option<&str>,
    ) -> Result<RunsResponse> {
        let mut url = self.url("/api/runs")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("limit", &limit.to_string());
            q.append_pair("offset", &offset.to_string());
            if let Some(w) = workflow {
                q.append_pair("workflow", w);
            }
            if let Some(s) = status {
                q.append_pair("status", s);
            }
        }
        Ok(self.send(Method::GET, url, None).await?.json().await?)
    }

    pub async fn fetch_run_detail(&self, run_id: &str) -> Result<RunDetail> {
        let url = self.url(&format!("/api/runs/{run_id}"))?;
        Ok(self.send(Method::GET, url, None).await?.json().await?)
    }

    pub async fn fetch_workflow_detail(&self, name: &str) -> Result<WorkflowInfo> {
        let mut url = self.url("/api/workflows")?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("cannot be base URL"))?
            .push(name);
        Ok(self.send(Method::GET, url, None).await?.json().await?)
    }

    pub async fn trigger_run(&self, workflow: &str) -> Result<CreateRunResponse> {
        #[derive(Default, Serialize)]
        struct Body<'a> {
            workflow: &'a str,
        }
        let url = self.url("/api/runs")?;
        let body = serde_json::to_value(Body { workflow })?;
        Ok(self
            .send(Method::POST, url, Some(&body))
            .await?
            .json()
            .await?)
    }

    pub async fn cancel_run(&self, run_id: &str) -> Result<()> {
        let url = self.url(&format!("/api/runs/{run_id}/cancel"))?;
        self.send(Method::POST, url, None).await?;
        Ok(())
    }

    pub async fn pause_run(&self, run_id: &str) -> Result<()> {
        let url = self.url(&format!("/api/runs/{run_id}/pause"))?;
        self.send(Method::POST, url, None).await?;
        Ok(())
    }

    pub async fn resume_run(&self, run_id: &str) -> Result<()> {
        let url = self.url(&format!("/api/runs/{run_id}/resume"))?;
        self.send(Method::POST, url, None).await?;
        Ok(())
    }

    pub async fn reload_workflow_definitions(&self) -> Result<serde_json::Value> {
        let url = self.url("/api/workflows/reload")?;
        Ok(self.send(Method::POST, url, None).await?.json().await?)
    }
}

// Helpers

impl ApiClient {
    fn url(&self, path: &str) -> Result<Url> {
        Url::parse(&format!("{}{}", self.base_url, path))
            .with_context(|| format!("building URL for {path}"))
    }

    /// Send with the token, if any. An error status is an error.
    async fn send(
        &self,
        method: Method,
        url: Url,
        body: Option<&serde_json::Value>,
    ) -> Result<reqwest::Response> {
        let mut request = self.client.request(method, url);
        if let Some(body) = body {
            request = request.json(body);
        }
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        Ok(request.send().await?.error_for_status()?)
    }
}
