use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ValidationOptions {
    pub contract_validation: bool,
}

impl ValidationOptions {
    pub const STRICT: Self = Self {
        contract_validation: true,
    };

    pub const RELAXED: Self = Self {
        contract_validation: false,
    };
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum FailurePolicy {
    #[default]
    Skip,
    Run,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WasmMode {
    #[default]
    Embedded,
    Cli,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct CustomExecutorDefinition {
    #[serde(rename = "type")]
    pub target: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ExecutorKind {
    Container,
    #[default]
    Process,
    Python,
    CloudRun,
    Library,
    Wasm,
    Custom(String),
}

impl ExecutorKind {
    pub fn is_builtin(&self) -> bool {
        !matches!(self, Self::Custom(_))
    }

    pub fn custom_alias(&self) -> Option<&str> {
        match self {
            Self::Custom(alias) => Some(alias),
            _ => None,
        }
    }
}

impl Serialize for ExecutorKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            Self::Container => "container",
            Self::Process => "process",
            Self::Python => "python",
            Self::CloudRun => "cloudrun",
            Self::Library => "library",
            Self::Wasm => "wasm",
            Self::Custom(alias) => alias,
        };
        serializer.serialize_str(value)
    }
}

impl<'de> Deserialize<'de> for ExecutorKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.trim().to_ascii_lowercase().as_str() {
            "container" => Ok(Self::Container),
            "process" => Ok(Self::Process),
            "python" => Ok(Self::Python),
            "cloudrun" => Ok(Self::CloudRun),
            "library" => Ok(Self::Library),
            "wasm" => Ok(Self::Wasm),
            _ => Ok(Self::Custom(value)),
        }
    }
}
