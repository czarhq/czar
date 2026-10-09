use czar_core::models::ExecutorType;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    pub task_id: Uuid,
    pub task_name: String,
    pub executor_type: RequestedExecutorType,
    pub params: Option<serde_json::Value>,
}

impl Default for ExecuteRequest {
    fn default() -> Self {
        Self {
            task_id: Uuid::nil(),
            task_name: "task".to_string(),
            executor_type: ExecutorType::Process.into(),
            params: None,
        }
    }
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestedExecutorType {
    Known(ExecutorType),
    Unknown(String),
}

impl RequestedExecutorType {
    pub fn known(&self) -> Option<ExecutorType> {
        match self {
            Self::Known(executor_type) => Some(*executor_type),
            Self::Unknown(_) => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Known(executor_type) => executor_type.as_str(),
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl From<ExecutorType> for RequestedExecutorType {
    fn from(value: ExecutorType) -> Self {
        Self::Known(value)
    }
}

impl fmt::Display for RequestedExecutorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for RequestedExecutorType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RequestedExecutorType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(ExecutorType::parse(&value)
            .map(Self::Known)
            .unwrap_or(Self::Unknown(value)))
    }
}
