use super::*;

impl Default for Workflow {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            schedule: None,
            types: BTreeMap::new(),
            vars: BTreeMap::new(),
            tasks: IndexMap::new(),
            hooks: IndexMap::new(),
        }
    }
}

impl Default for TaskDefinition {
    fn default() -> Self {
        Self {
            executor: ExecutorKind::Process,
            image: None,
            file: None,
            config_file: None,
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
            timeout: default_timeout_secs(),
            retries: default_retries(),
            retry_delay: None,
            on_failure: FailurePolicy::default(),
            input_type: None,
            output_type: None,
            validate_output: default_true(),
            when: None,
            parallel: None,
            foreach: None,
            concurrency: None,
            parallel_mode: ParallelMode::default(),
        }
    }
}

impl Default for HookDefinition {
    fn default() -> Self {
        Self {
            on: HookEvent::TaskStart,
            tasks: None,
            executor: ExecutorKind::Process,
            file: None,
            command: None,
            job: None,
            module: None,
            function: None,
            timeout: default_timeout_secs(),
        }
    }
}

impl Default for WorkflowDefinition {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            schedule: None,
            types: BTreeMap::new(),
            vars: BTreeMap::new(),
            imports: Vec::new(),
            task_groups: BTreeMap::new(),
            tasks: IndexMap::new(),
            flows: IndexMap::new(),
            hooks: IndexMap::new(),
            executor_registry: BTreeMap::new(),
        }
    }
}

impl Default for TaskTemplateDefinition {
    fn default() -> Self {
        Self {
            executor: None,
            image: None,
            file: None,
            config_file: None,
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
            timeout: default_timeout_secs(),
            retries: default_retries(),
            retry_delay: None,
            on_failure: FailurePolicy::default(),
            input_type: None,
            output_type: None,
            validate_output: default_true(),
            when: None,
            parallel: None,
            foreach: None,
            concurrency: None,
            parallel_mode: ParallelMode::default(),
        }
    }
}
