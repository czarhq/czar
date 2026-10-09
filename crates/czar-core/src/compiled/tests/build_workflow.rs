use super::*;

fn compiled_fixture() -> CompiledWorkflow {
    CompiledWorkflow {
        name: "fixture".to_string(),
        vars: std::collections::BTreeMap::new(),
        types: std::collections::BTreeMap::new(),
        tasks: vec![
            CompiledTask {
                name: "process_task".to_string(),
                executor: ExecutorKind::Process,
                image: None,
                file: None,
                config_file: None,
                python_path: None,
                command: Some("echo hello".to_string()),
                job: None,
                module: None,
                function: None,
                custom_executor: None,
                inputs: serde_json::Value::Null,
                env: serde_json::Value::Null,
                secrets: Vec::new(),
                depends_on: vec![],
                timeout: 30,
                retries: 2,
                retry_delay: None,
                on_failure: crate::workflow::FailurePolicy::Run,
                signature: None,
                input_type: None,
                output_type: None,
                validate_output: true,
                when: None,
                parallel: None,
                foreach: None,
                concurrency: None,
                parallel_mode: Default::default(),
                args: vec![],
                wasm_mode: Default::default(),
            },
            CompiledTask {
                name: "python_task".to_string(),
                executor: ExecutorKind::Python,
                image: None,
                file: Some(PathBuf::from("tasks/example.py")),
                config_file: None,
                python_path: Some(PathBuf::from("/tmp/python-project")),
                command: None,
                job: None,
                module: Some("tasks.example".to_string()),
                function: Some("run".to_string()),
                custom_executor: None,
                inputs: serde_json::json!({"k": {"const": "v"}}),
                env: serde_json::json!({
                    "LOG_LEVEL": "debug",
                    "FEATURE_ENABLED": true
                }),
                secrets: Vec::new(),
                depends_on: vec![0],
                timeout: 45,
                retries: 1,
                retry_delay: None,
                on_failure: crate::workflow::FailurePolicy::Skip,
                signature: Some(serde_json::json!({"args": []})),
                input_type: None,
                output_type: None,
                validate_output: true,
                when: None,
                parallel: None,
                foreach: None,
                concurrency: None,
                parallel_mode: Default::default(),
                args: vec![],
                wasm_mode: Default::default(),
            },
            CompiledTask {
                name: "cloud_task".to_string(),
                executor: ExecutorKind::CloudRun,
                image: None,
                file: None,
                config_file: None,
                python_path: None,
                command: None,
                job: Some("cloud-job".to_string()),
                module: None,
                function: None,
                custom_executor: None,
                inputs: serde_json::Value::Null,
                env: serde_json::Value::Null,
                secrets: Vec::new(),
                depends_on: vec![0, 1],
                timeout: 60,
                retries: 0,
                retry_delay: None,
                on_failure: crate::workflow::FailurePolicy::Skip,
                signature: None,
                input_type: None,
                output_type: None,
                validate_output: true,
                when: None,
                parallel: None,
                foreach: None,
                concurrency: None,
                parallel_mode: Default::default(),
                args: vec![],
                wasm_mode: Default::default(),
            },
            CompiledTask {
                name: "library_task".to_string(),
                executor: ExecutorKind::Library,
                image: None,
                file: Some(PathBuf::from("target/libtask.so")),
                config_file: None,
                python_path: None,
                command: None,
                job: None,
                module: None,
                function: None,
                custom_executor: None,
                inputs: serde_json::Value::Null,
                env: serde_json::Value::Null,
                secrets: Vec::new(),
                depends_on: vec![2],
                timeout: 15,
                retries: 0,
                retry_delay: None,
                on_failure: crate::workflow::FailurePolicy::Skip,
                signature: None,
                input_type: None,
                output_type: None,
                validate_output: true,
                when: None,
                parallel: None,
                foreach: None,
                concurrency: None,
                parallel_mode: Default::default(),
                args: vec![],
                wasm_mode: Default::default(),
            },
            CompiledTask {
                name: "wasm_task".to_string(),
                executor: ExecutorKind::Wasm,
                image: None,
                file: Some(PathBuf::from("target/task.wasm")),
                config_file: None,
                python_path: None,
                command: None,
                job: None,
                module: None,
                function: Some("_start".to_string()),
                custom_executor: None,
                inputs: serde_json::json!({"name": {"const": "World"}}),
                env: serde_json::Value::Null,
                secrets: Vec::new(),
                depends_on: vec![3],
                timeout: 20,
                retries: 4,
                retry_delay: Some(9),
                on_failure: crate::workflow::FailurePolicy::Skip,
                signature: None,
                input_type: Some(serde_json::json!({"name": "str"})),
                output_type: Some(serde_json::json!({"ok": "bool"})),
                validate_output: true,
                when: None,
                parallel: None,
                foreach: None,
                concurrency: None,
                parallel_mode: Default::default(),
                args: vec![],
                wasm_mode: Default::default(),
            },
            CompiledTask {
                name: "container_task".to_string(),
                executor: ExecutorKind::Container,
                image: Some("alpine:3.20".to_string()),
                file: None,
                config_file: None,
                python_path: None,
                command: Some("sh".to_string()),
                job: None,
                module: None,
                function: None,
                custom_executor: None,
                inputs: serde_json::json!({"name": {"const": "Czar"}}),
                env: serde_json::json!({"GREETING": "hello"}),
                secrets: Vec::new(),
                depends_on: vec![4],
                timeout: 25,
                retries: 1,
                retry_delay: None,
                on_failure: crate::workflow::FailurePolicy::Skip,
                signature: None,
                input_type: None,
                output_type: None,
                validate_output: true,
                when: None,
                parallel: None,
                foreach: None,
                concurrency: None,
                parallel_mode: Default::default(),
                args: vec!["-c".to_string(), "echo ok".to_string()],
                wasm_mode: Default::default(),
            },
        ],
        name_index: Default::default(),
        topo: vec![0, 1, 2, 3, 4, 5],
        root: PathBuf::from("/tmp/czar-workflow"),
        hooks: vec![],
    }
}

#[test]
fn test_build_workflow_tasks_maps_dependencies_and_executor_types() {
    let compiled = compiled_fixture();
    let tasks = build_workflow_tasks(&compiled);

    assert_eq!(tasks.len(), 6);
    assert_eq!(tasks[0].executor_type, "process");
    assert_eq!(tasks[1].executor_type, "python");
    assert_eq!(tasks[2].executor_type, "cloudrun");
    assert_eq!(tasks[3].executor_type, "library");
    assert_eq!(tasks[4].executor_type, "wasm");
    assert_eq!(tasks[5].executor_type, "container");

    assert_eq!(tasks[0].depends_on, Vec::<String>::new());
    assert_eq!(tasks[1].depends_on, vec!["process_task".to_string()]);
    assert_eq!(
        tasks[2].depends_on,
        vec!["process_task".to_string(), "python_task".to_string()]
    );
    assert_eq!(tasks[3].depends_on, vec!["cloud_task".to_string()]);
    assert_eq!(tasks[4].depends_on, vec!["library_task".to_string()]);
    assert_eq!(tasks[5].depends_on, vec!["wasm_task".to_string()]);
}

#[test]
fn test_build_workflow_tasks_populates_executor_specific_params() {
    let compiled = compiled_fixture();
    let tasks = build_workflow_tasks(&compiled);

    let process_params = &tasks[0].params;
    assert_eq!(
        process_params.get("command"),
        Some(&serde_json::json!("echo hello"))
    );
    assert_eq!(
        process_params.get("max_retries"),
        Some(&serde_json::json!(2))
    );
    assert_eq!(
        process_params.get("timeout_seconds"),
        Some(&serde_json::json!(30))
    );
    assert_eq!(
        process_params.get("on_failure"),
        Some(&serde_json::json!("run"))
    );

    let python_params = &tasks[1].params;
    assert_eq!(
        python_params.get("task_file"),
        Some(&serde_json::json!("tasks/example.py"))
    );
    assert_eq!(
        python_params.get("task_module"),
        Some(&serde_json::json!("tasks.example"))
    );
    assert_eq!(
        python_params.get("task_function"),
        Some(&serde_json::json!("run"))
    );
    assert_eq!(
        python_params.get("python_path"),
        Some(&serde_json::json!("/tmp/python-project"))
    );
    assert_eq!(
        python_params.get("task_bindings"),
        Some(&serde_json::json!({"k": {"const": "v"}}))
    );

    let cloud_params = &tasks[2].params;
    assert_eq!(
        cloud_params.get("job_name"),
        Some(&serde_json::json!("cloud-job"))
    );

    let library_params = &tasks[3].params;
    assert_eq!(
        library_params.get("library_path"),
        Some(&serde_json::json!("target/libtask.so"))
    );

    let wasm_params = &tasks[4].params;
    assert_eq!(
        wasm_params.get("task_bindings"),
        Some(&serde_json::json!({"name": {"const": "World"}}))
    );
    assert_eq!(
        wasm_params.get("wasm_module_path"),
        Some(&serde_json::json!("target/task.wasm"))
    );
    assert_eq!(
        wasm_params.get("wasm_entrypoint"),
        Some(&serde_json::json!("_start"))
    );
    assert_eq!(wasm_params.get("max_retries"), Some(&serde_json::json!(4)));
    assert_eq!(
        wasm_params.get("retry_delay_seconds"),
        Some(&serde_json::json!(9))
    );
    assert_eq!(
        wasm_params.get("timeout_seconds"),
        Some(&serde_json::json!(20))
    );
    assert_eq!(
        tasks[4].signature,
        Some(serde_json::json!({
            "input_type": {"name": "str"},
            "output_type": {"ok": "bool"}
        }))
    );

    let container_params = &tasks[5].params;
    assert_eq!(
        container_params.get("container_image"),
        Some(&serde_json::json!("alpine:3.20"))
    );
    assert_eq!(
        container_params.get("command"),
        Some(&serde_json::json!("sh"))
    );
    assert_eq!(
        container_params.get("task_args"),
        Some(&serde_json::json!(["-c", "echo ok"]))
    );
}

#[test]
fn test_build_workflow_tasks_resolves_relative_process_command_from_workflow_root() {
    let mut compiled = compiled_fixture();
    compiled.tasks[0].command = Some("bin/process-task".to_string());
    compiled.root = PathBuf::from("/tmp/czar-workflow");

    let tasks = build_workflow_tasks(&compiled);

    assert_eq!(
        tasks[0].params.get("command"),
        Some(&serde_json::json!("/tmp/czar-workflow/bin/process-task"))
    );
}

#[test]
fn test_build_workflow_tasks_omits_output_signature_when_validate_output_disabled() {
    let mut compiled = compiled_fixture();
    compiled.tasks[4].validate_output = false;

    let tasks = build_workflow_tasks(&compiled);

    assert_eq!(
        tasks[4].signature,
        Some(serde_json::json!({
            "input_type": {"name": "str"}
        }))
    );
}

#[test]
fn test_build_workflow_tasks_maps_process_file_into_command() {
    let compiled = CompiledWorkflow {
        name: "wf".to_string(),
        vars: std::collections::BTreeMap::new(),
        types: std::collections::BTreeMap::new(),
        tasks: vec![CompiledTask {
            name: "from-file".to_string(),
            executor: ExecutorKind::Process,
            image: None,
            file: Some(PathBuf::from("/tmp/task.py")),
            config_file: None,
            python_path: None,
            command: None,
            job: None,
            module: None,
            function: None,
            custom_executor: None,
            inputs: serde_json::Value::Null,
            env: serde_json::Value::Null,
            secrets: Vec::new(),
            depends_on: vec![],
            timeout: 5,
            retries: 0,
            retry_delay: None,
            on_failure: crate::workflow::FailurePolicy::Skip,
            signature: None,
            input_type: None,
            output_type: None,
            validate_output: true,
            when: None,
            parallel: None,
            foreach: None,
            concurrency: None,
            parallel_mode: Default::default(),
            args: vec![],
            wasm_mode: Default::default(),
        }],
        name_index: indexmap::IndexMap::new(),
        topo: vec![0],
        root: PathBuf::from("/tmp"),
        hooks: vec![],
    };

    let tasks = build_workflow_tasks(&compiled);
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].params["command"],
        serde_json::json!("/tmp/task.py")
    );
}

#[test]
fn test_resolve_process_command_keeps_commands_with_whitespace() {
    let root = Path::new("/tmp");
    assert_eq!(
        resolve_process_command("echo hello world", root),
        "echo hello world"
    );
}

#[test]
fn test_resolve_process_command_keeps_absolute_paths() {
    let root = Path::new("/tmp");
    assert_eq!(
        resolve_process_command("/usr/bin/python", root),
        "/usr/bin/python"
    );
}

#[test]
fn test_resolve_process_command_keeps_commands_without_slash() {
    let root = Path::new("/tmp");
    assert_eq!(resolve_process_command("python", root), "python");
}

#[test]
fn test_resolve_process_command_resolves_relative_path() {
    let root = Path::new("/tmp/workflow");
    assert_eq!(
        resolve_process_command("bin/task", root),
        "/tmp/workflow/bin/task"
    );
}
