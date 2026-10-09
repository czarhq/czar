use super::*;

#[derive(Debug, Clone, Default)]
pub struct CompiledHook {
    /// Arbitrary name for logging and identification.
    pub name: String,
    /// The event that triggers this hook.
    pub event: HookEvent,
    /// Optional task name filter (None means all tasks).
    pub tasks_filter: Option<HookTaskFilter>,
    /// Executor type.
    pub executor: ExecutorKind,
    pub file: Option<PathBuf>,
    pub python_path: Option<PathBuf>,
    pub command: Option<String>,
    pub job: Option<String>,
    pub module: Option<String>,
    pub function: Option<String>,
    pub timeout: u64,
}

#[derive(Debug, Clone)]
pub struct CompiledWorkflow {
    pub name: String,
    pub vars: BTreeMap<String, String>,
    pub types: BTreeMap<String, serde_json::Value>,
    pub tasks: Vec<CompiledTask>,
    pub name_index: IndexMap<String, usize>,
    pub topo: Vec<usize>,
    pub root: PathBuf,
    pub hooks: Vec<CompiledHook>,
}

impl Default for CompiledWorkflow {
    fn default() -> Self {
        Self {
            name: String::new(),
            vars: BTreeMap::new(),
            types: BTreeMap::new(),
            tasks: Vec::new(),
            name_index: IndexMap::new(),
            topo: Vec::new(),
            root: PathBuf::from("."),
            hooks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompiledTask {
    pub name: String,
    pub executor: ExecutorKind,
    pub image: Option<String>,
    pub file: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub python_path: Option<PathBuf>,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub wasm_mode: WasmMode,
    pub job: Option<String>,
    pub module: Option<String>,
    pub function: Option<String>,
    pub custom_executor: Option<CustomExecutorDefinition>,
    pub inputs: serde_json::Value,
    pub env: serde_json::Value,
    pub secrets: Vec<SecretFileMount>,
    pub depends_on: Vec<usize>,
    pub timeout: u64,
    pub retries: u32,
    pub retry_delay: Option<u64>,
    pub on_failure: FailurePolicy,
    pub signature: Option<serde_json::Value>,
    pub input_type: Option<serde_json::Value>,
    pub output_type: Option<serde_json::Value>,
    pub validate_output: bool,
    pub when: Option<String>,
    pub parallel: Option<ParallelSpec>,
    pub foreach: Option<String>,
    pub concurrency: Option<u32>,
    pub parallel_mode: ParallelMode,
}

impl Default for CompiledTask {
    fn default() -> Self {
        Self {
            name: String::new(),
            executor: ExecutorKind::Process,
            image: None,
            file: None,
            config_file: None,
            python_path: None,
            command: None,
            args: Vec::new(),
            wasm_mode: WasmMode::default(),
            job: None,
            module: None,
            function: None,
            custom_executor: None,
            inputs: serde_json::Value::Null,
            env: serde_json::Value::Null,
            secrets: Vec::new(),
            depends_on: Vec::new(),
            timeout: 300,
            retries: 0,
            retry_delay: None,
            on_failure: FailurePolicy::default(),
            signature: None,
            input_type: None,
            output_type: None,
            validate_output: true,
            when: None,
            parallel: None,
            foreach: None,
            concurrency: None,
            parallel_mode: ParallelMode::default(),
        }
    }
}
