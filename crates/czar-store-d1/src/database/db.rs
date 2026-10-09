use anyhow::{Context, Result, anyhow};
use serde_json::Value;

use crate::D1Config;
use crate::client::{D1Client, QueryParam, RowMap};
use crate::migrations;

#[derive(Clone)]
pub struct D1Database {
    pub(crate) client: D1Client,
}

impl Default for D1Database {
    fn default() -> Self {
        Self {
            client: D1Client::new(&D1Config::default()).expect("default D1 client should build"),
        }
    }
}

impl D1Database {
    pub async fn new(config: &D1Config) -> Result<Self> {
        Ok(Self {
            client: D1Client::new(config)?,
        })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        migrations::run_migrations(&self.client).await
    }

    pub(crate) async fn execute(&self, sql: &str, params: Vec<QueryParam>) -> Result<()> {
        self.client.execute(sql, params).await?;
        Ok(())
    }

    pub(crate) async fn fetch_one<T, F>(
        &self,
        sql: &str,
        params: Vec<QueryParam>,
        map: F,
    ) -> Result<T>
    where
        F: FnOnce(RowMap) -> Result<T>,
    {
        let mut rows = self.client.execute(sql, params).await?.results.into_iter();
        let row = rows
            .next()
            .ok_or_else(|| anyhow!("row not found for SQL: {sql}"))?;
        map(row)
    }

    pub(crate) async fn fetch_all<T, F>(
        &self,
        sql: &str,
        params: Vec<QueryParam>,
        map: F,
    ) -> Result<Vec<T>>
    where
        F: Fn(RowMap) -> Result<T>,
    {
        self.client
            .execute(sql, params)
            .await?
            .results
            .into_iter()
            .map(map)
            .collect()
    }

    pub(crate) async fn scalar_i64(
        &self,
        sql: &str,
        params: Vec<QueryParam>,
        alias: &str,
    ) -> Result<i64> {
        let row = self
            .fetch_one(sql, params, Ok)
            .await
            .with_context(|| format!("expected scalar row {alias}"))?;
        match row.get(alias) {
            Some(Value::Number(value)) => value
                .as_i64()
                .ok_or_else(|| anyhow!("invalid scalar value for {alias}")),
            Some(Value::String(value)) => value
                .parse::<i64>()
                .with_context(|| format!("invalid integer string for {alias}")),
            other => Err(anyhow!("missing scalar column {alias}: {other:?}")),
        }
    }
}
