use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::client::{QueryParam, RowMap};

pub(crate) fn uuid_param(value: Uuid) -> QueryParam {
    QueryParam::Text(value.to_string())
}

pub(crate) fn opt_uuid(value: Option<Uuid>) -> QueryParam {
    value.map(uuid_param).unwrap_or(QueryParam::Null)
}

pub(crate) fn opt_text(value: Option<&str>) -> QueryParam {
    value
        .map(|value| QueryParam::Text(value.to_string()))
        .unwrap_or(QueryParam::Null)
}

pub(crate) fn opt_text_owned(value: Option<String>) -> QueryParam {
    value.map(QueryParam::Text).unwrap_or(QueryParam::Null)
}

pub(crate) fn datetime_param(value: DateTime<Utc>) -> QueryParam {
    QueryParam::Text(value.to_rfc3339_opts(SecondsFormat::Secs, true))
}

pub(crate) fn opt_datetime(value: Option<DateTime<Utc>>) -> QueryParam {
    value.map(datetime_param).unwrap_or(QueryParam::Null)
}

pub(crate) fn opt_json(value: Option<Value>) -> QueryParam {
    value
        .map(|value| QueryParam::Text(value.to_string()))
        .unwrap_or(QueryParam::Null)
}

pub(crate) fn opt_i32_param(value: Option<i32>) -> QueryParam {
    value.map(QueryParam::from).unwrap_or(QueryParam::Null)
}

pub(crate) fn json_text<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("serializing JSON into D1 parameter should not fail")
}

pub(crate) fn scalar_from_row(row: &RowMap, key: &str) -> Result<i64> {
    match row.get(key) {
        Some(Value::Number(value)) => value
            .as_i64()
            .ok_or_else(|| anyhow!("invalid integer for {key}")),
        Some(Value::String(value)) => value
            .parse::<i64>()
            .with_context(|| format!("invalid integer string for {key}")),
        other => Err(anyhow!("missing numeric column {key}: {other:?}")),
    }
}
