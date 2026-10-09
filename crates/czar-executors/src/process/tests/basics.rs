use super::*;

#[test]
fn test_build_env_vars_extracts_known_fields() {
    let params = serde_json::json!({
        "task_file": "tasks/job.py",
        "task_input": {"value": 42},
        "task_module": "pkg.tasks",
        "task_function": "run",
        "python_path": "/tmp/python-root",
        "runner_path": "/tmp/runner.py",
        "upstream": {"step_a": {"ok": true}},
        "env": {"CZAR_ATTEMPT": 3, "DEBUG": true},
        "string_meta": "x",
        "numeric_meta": 7,
        "bool_meta": true
    });

    let (
        env_vars,
        secret_file_mounts,
        task_file,
        config_file,
        task_module,
        task_function,
        custom_executor,
        task_input,
        runner_path,
        python_path,
        _task_args,
        container_image,
    ) = build_env_vars(Some(params)).expect("build env vars");

    assert_eq!(task_file.expect("task_file"), PathBuf::from("tasks/job.py"));
    assert_eq!(task_module.as_deref(), Some("pkg.tasks"));
    assert_eq!(task_function.as_deref(), Some("run"));
    assert!(config_file.is_none());
    assert!(custom_executor.is_none());
    assert_eq!(
        python_path.expect("python_path"),
        PathBuf::from("/tmp/python-root")
    );
    assert_eq!(
        runner_path.expect("runner_path"),
        PathBuf::from("/tmp/runner.py")
    );
    assert_eq!(
        task_input.expect("task_input"),
        serde_json::json!({"value": 42})
    );

    assert_eq!(env_vars.get("string_meta").map(String::as_str), Some("x"));
    assert_eq!(env_vars.get("numeric_meta").map(String::as_str), Some("7"));
    assert_eq!(env_vars.get("bool_meta").map(String::as_str), Some("true"));
    assert_eq!(env_vars.get("CZAR_ATTEMPT").map(String::as_str), Some("3"));
    assert_eq!(env_vars.get("DEBUG").map(String::as_str), Some("true"));
    assert!(secret_file_mounts.is_empty());
    assert!(env_vars.contains_key("CZAR_INPUT_JSON"));
    assert!(env_vars.contains_key("CZAR_UPSTREAM_JSON"));
    assert!(container_image.is_none());
}

#[test]
fn test_build_env_vars_with_none_returns_defaults() {
    let (
        env_vars,
        secret_file_mounts,
        task_file,
        config_file,
        task_module,
        task_function,
        custom_executor,
        task_input,
        runner_path,
        python_path,
        task_args,
        container_image,
    ) = build_env_vars(None).expect("build env vars");
    assert!(env_vars.is_empty());
    assert!(secret_file_mounts.is_empty());
    assert!(task_file.is_none());
    assert!(config_file.is_none());
    assert!(task_module.is_none());
    assert!(task_function.is_none());
    assert!(custom_executor.is_none());
    assert!(task_input.is_none());
    assert!(runner_path.is_none());
    assert!(python_path.is_none());
    assert!(task_args.is_empty());
    assert!(container_image.is_none());
}

#[test]
fn test_build_env_vars_extracts_literal_env_values() {
    let (
        env_vars,
        secret_file_mounts,
        _task_file,
        _config_file,
        _task_module,
        _task_function,
        _custom_executor,
        _task_input,
        _runner_path,
        _python_path,
        _task_args,
        container_image,
    ) = build_env_vars(Some(serde_json::json!({
        "env": {
            "API_KEY": "already-injected",
            "LOG_LEVEL": "debug"
        }
    })))
    .expect("build env vars");

    assert_eq!(env_vars.get("LOG_LEVEL").map(String::as_str), Some("debug"));
    assert_eq!(
        env_vars.get("API_KEY").map(String::as_str),
        Some("already-injected")
    );
    assert!(secret_file_mounts.is_empty());
    assert!(container_image.is_none());
}

#[test]
fn test_build_env_vars_extracts_secret_file_mounts() {
    let (
        env_vars,
        secret_file_mounts,
        _task_file,
        _config_file,
        _task_module,
        _task_function,
        _custom_executor,
        _task_input,
        _runner_path,
        _python_path,
        _task_args,
        container_image,
    ) = build_env_vars(Some(serde_json::json!({
        "secrets": [
            {
                "name": "service-account",
                "secret": "projects/demo/secrets/api-key",
                "path": ".czar/secrets/api-key.json"
            }
        ]
    })))
    .expect("build env vars");

    assert!(env_vars.is_empty());
    assert_eq!(secret_file_mounts.len(), 1);
    assert_eq!(secret_file_mounts[0].name, "service-account");
    assert_eq!(
        secret_file_mounts[0].path,
        PathBuf::from(".czar/secrets/api-key.json")
    );
    assert!(container_image.is_none());
}

#[test]
fn test_build_env_vars_extracts_container_image() {
    let (
        env_vars,
        secret_file_mounts,
        _task_file,
        _config_file,
        _task_module,
        _task_function,
        _custom_executor,
        _task_input,
        _runner_path,
        _python_path,
        task_args,
        container_image,
    ) = build_env_vars(Some(serde_json::json!({
        "container_image": "alpine:3.20",
        "task_args": ["-c", "echo hi"]
    })))
    .expect("build env vars");

    assert!(env_vars.is_empty());
    assert!(secret_file_mounts.is_empty());
    assert_eq!(task_args, vec!["-c".to_string(), "echo hi".to_string()]);
    assert_eq!(container_image.as_deref(), Some("alpine:3.20"));
}

#[test]
fn test_resolve_command_path_behavior() {
    assert_eq!(
        resolve_command_path(".", "/usr/bin/echo"),
        "/usr/bin/echo".to_string()
    );
    assert_eq!(
        resolve_command_path(".", "echo hello"),
        "echo hello".to_string()
    );
    assert_eq!(
        resolve_command_path(".", "scripts/run.sh"),
        "scripts/run.sh".to_string()
    );
    assert_eq!(resolve_command_path(".", "my-task"), "my-task".to_string());
}

#[test]
fn test_build_python_command_uses_uv_when_pyproject_exists() {
    let base = std::env::temp_dir().join(format!("czar-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");
    write_pyproject(&base);

    let (cmd, uses_uv) = build_python_command(Some(&base)).expect("python command");
    assert!(uses_uv);
    assert_eq!(cmd.as_std().get_program().to_string_lossy(), "uv");
    assert_eq!(cmd.as_std().get_current_dir(), Some(base.as_path()));
    assert_eq!(
        cmd.as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        vec!["run".to_string(), "python".to_string()]
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn test_build_python_command_requires_pyproject() {
    let base = std::env::temp_dir().join(format!("czar-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create dir");

    let err = build_python_command(Some(&base)).expect_err("missing pyproject must fail");
    assert!(err.to_string().contains("pyproject.toml"));

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn test_build_python_command_requires_python_path() {
    let err = build_python_command(None).expect_err("missing python_path must fail");
    assert!(
        err.to_string()
            .contains("python_path resolved at compile time")
    );
}
