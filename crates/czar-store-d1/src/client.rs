use anyhow::{Context, Result, anyhow, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::D1Config;

pub(crate) type RowMap = Map<String, Value>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct D1QueryRequest {
    pub sql: String,
    #[serde(default)]
    pub params: Vec<QueryParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum QueryParam {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(String),
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct D1Envelope {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub errors: Vec<D1ResponseInfo>,
    #[serde(default)]
    pub messages: Vec<D1ResponseInfo>,
    #[serde(default)]
    pub result: Vec<D1QueryResult>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct D1QueryResult {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub results: Vec<RowMap>,
    #[serde(default)]
    pub meta: D1QueryMeta,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct D1QueryMeta {
    #[serde(default)]
    pub changed_db: bool,
    #[serde(default)]
    pub changes: u64,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub last_row_id: Option<u64>,
    #[serde(default)]
    pub rows_read: u64,
    #[serde(default)]
    pub rows_written: u64,
    #[serde(default)]
    pub served_by_colo: Option<String>,
    #[serde(default)]
    pub served_by_primary: Option<bool>,
    #[serde(default)]
    pub served_by_region: Option<String>,
    #[serde(default)]
    pub size_after: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct D1ResponseInfo {
    pub code: Option<u64>,
    pub message: Option<String>,
}

#[derive(Clone)]
pub(crate) struct D1Client {
    http: Client,
    account_id: String,
    database_id: String,
    api_token: String,
    base_url: String,
}

impl Default for D1Client {
    fn default() -> Self {
        Self::new(&D1Config::default()).expect("default D1 client should build")
    }
}

impl D1Client {
    pub(crate) fn new(config: &D1Config) -> Result<Self> {
        let http = Client::builder().build()?;
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.cloudflare.com/client/v4".to_string())
            .trim_end_matches('/')
            .to_string();

        Ok(Self {
            http,
            account_id: config.account_id.clone(),
            database_id: config.database_id.clone(),
            api_token: config.api_token.clone(),
            base_url,
        })
    }

    pub(crate) async fn execute(
        &self,
        sql: &str,
        params: Vec<QueryParam>,
    ) -> Result<D1QueryResult> {
        let request = D1QueryRequest {
            sql: sql.to_string(),
            params,
        };

        let response = self
            .http
            .post(self.query_url())
            .bearer_auth(&self.api_token)
            .json(&request)
            .send()
            .await
            .with_context(|| format!("failed to execute D1 SQL: {sql}"))?;

        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            bail!("D1 HTTP error {status}: {body}");
        }

        let envelope: D1Envelope =
            serde_json::from_str(&body).context("failed to deserialize D1 response")?;
        if !envelope.success {
            return Err(anyhow!(format_messages(
                &envelope.errors,
                &envelope.messages
            )));
        }

        let mut results = envelope.result.into_iter();
        let result = results.next().context("D1 response missing query result")?;
        if !result.success {
            bail!(
                "D1 query reported failure for SQL: {sql}; meta rows_read={}, rows_written={}",
                result.meta.rows_read,
                result.meta.rows_written
            );
        }

        Ok(result)
    }

    fn query_url(&self) -> String {
        format!(
            "{}/accounts/{}/d1/database/{}/query",
            self.base_url, self.account_id, self.database_id
        )
    }
}

fn format_messages(errors: &[D1ResponseInfo], messages: &[D1ResponseInfo]) -> String {
    let combined = errors
        .iter()
        .chain(messages.iter())
        .filter_map(|info| match (info.code, info.message.as_deref()) {
            (Some(code), Some(message)) => Some(format!("{code}: {message}")),
            (_, Some(message)) => Some(message.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if combined.is_empty() {
        "Cloudflare D1 request failed without error details".to_string()
    } else {
        combined.join("; ")
    }
}

impl From<&str> for QueryParam {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for QueryParam {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<i32> for QueryParam {
    fn from(value: i32) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i64> for QueryParam {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<usize> for QueryParam {
    fn from(value: usize) -> Self {
        Self::Integer(i64::try_from(value).unwrap_or(i64::MAX))
    }
}

impl From<bool> for QueryParam {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
