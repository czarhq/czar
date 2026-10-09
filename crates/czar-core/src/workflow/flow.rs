use super::*;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct FlowDefinition {
    #[serde(default)]
    pub description: Option<String>,
    pub tasks: FlowTaskSet,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FlowTaskSet {
    Map(IndexMap<String, FlowTaskEntry>),
    List(Vec<String>),
    Star(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FlowTaskEntry {
    Config(FlowTaskDefinition),
    Empty(()),
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct FlowTaskDefinition {
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub inputs: Option<serde_json::Value>,
    #[serde(default)]
    pub env: Option<serde_json::Value>,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub retries: Option<u32>,
    #[serde(default)]
    pub retry_delay: Option<u64>,
    #[serde(default)]
    pub on_failure: Option<FailurePolicy>,
}

impl Default for FlowTaskSet {
    fn default() -> Self {
        Self::Map(IndexMap::new())
    }
}

impl Default for FlowTaskEntry {
    fn default() -> Self {
        Self::Config(FlowTaskDefinition::default())
    }
}
