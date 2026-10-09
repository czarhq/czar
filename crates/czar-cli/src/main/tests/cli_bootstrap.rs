use super::*;
use uuid::Uuid;

#[test]
fn test_cli_parse_defaults() {
    let cli = Cli::parse_from(["czar", "init"]);
    assert_eq!(cli.database.db_backend, None);
    assert_eq!(cli.database.database_url, None);
    assert_eq!(cli.database.d1_account_id, None);
    assert_eq!(cli.database.d1_database_id, None);
    assert_eq!(cli.database.d1_api_token, None);
    assert_eq!(cli.database.d1_base_url, None);
}

#[test]
fn test_cli_parses_run_start_input_flags() {
    let cli = Cli::try_parse_from([
        "czar",
        "run",
        "start",
        "wf",
        "--input",
        "date=2024-01-15",
        "--inputs",
        "limit=5",
    ])
    .expect("--input and --inputs should parse");
    let Commands::Run(commands::Run {
        action: Some(commands::run::RunAction::Start(start)),
        ..
    }) = cli.command
    else {
        panic!("expected `run start`");
    };
    assert_eq!(
        start.inputs,
        vec![
            ("date".to_string(), serde_json::json!("2024-01-15")),
            ("limit".to_string(), serde_json::json!(5)),
        ]
    );
}

#[test]
fn test_init_tracing_is_idempotent() {
    init_tracing().expect("first init_tracing call should succeed");
    init_tracing().expect("second init_tracing call should also succeed");
}

#[test]
fn test_resolve_database_url_prefers_env_var() {
    let _guard = env_lock().lock().expect("env lock");
    let original = std::env::var("DATABASE_URL").ok();
    unsafe {
        std::env::set_var("DATABASE_URL", "postgres://previous-db");
    }
    let prev = std::env::var("DATABASE_URL").ok();
    unsafe {
        std::env::set_var("DATABASE_URL", "postgres://env-db");
    }
    let resolved = resolve_database_url(None, DatabaseBackend::Postgres).expect("database url");
    assert_eq!(resolved, "postgres://env-db");

    match prev {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
    match original {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
}

#[test]
fn test_resolve_database_url_falls_back_to_default() {
    let _guard = env_lock().lock().expect("env lock");
    let original = std::env::var("DATABASE_URL").ok();
    unsafe {
        std::env::set_var("DATABASE_URL", "postgres://previous-db");
    }
    let prev = std::env::var("DATABASE_URL").ok();
    unsafe {
        std::env::remove_var("DATABASE_URL");
    }
    #[cfg(feature = "postgres")]
    assert_eq!(
        resolve_database_url(None, DatabaseBackend::Postgres).expect("database url"),
        DEFAULT_POSTGRES_DATABASE_URL
    );

    #[cfg(feature = "sqlite")]
    assert_eq!(
        resolve_database_url(None, DatabaseBackend::Sqlite).expect("database url"),
        czar_store::bootstrap::default_sqlite_database_url()
    );

    if let Some(v) = prev {
        unsafe {
            std::env::set_var("DATABASE_URL", v);
        }
    }
    match original {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
}

#[test]
fn test_resolve_database_url_prefers_cli_value() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("DATABASE_URL").ok();
    unsafe {
        std::env::set_var("DATABASE_URL", "postgres://env-db");
    }

    let Some(backend) = compiled_backends()
        .into_iter()
        .find(|backend| *backend != DatabaseBackend::D1)
    else {
        match prev {
            Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
            None => unsafe { std::env::remove_var("DATABASE_URL") },
        }
        return;
    };

    let resolved =
        resolve_database_url(Some("postgres://cli-db".to_string()), backend).expect("database url");
    assert_eq!(resolved, "postgres://cli-db");

    match prev {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
}

#[test]
fn test_resolve_database_url_for_d1_requires_d1_config_vars() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("DATABASE_URL").ok();
    unsafe {
        std::env::remove_var("DATABASE_URL");
    }

    let err = resolve_database_url(None, DatabaseBackend::D1)
        .expect_err("d1 should not use database url");
    assert!(err.to_string().contains("D1 does not use DATABASE_URL"));

    match prev {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
}

#[test]
fn test_resolve_backend_prefers_cli_over_env() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("CZAR_DB_BACKEND").ok();
    let explicit_backend = compiled_backends()
        .first()
        .copied()
        .expect("compiled backend");
    unsafe {
        std::env::set_var("CZAR_DB_BACKEND", "invalid-backend");
    }

    let backend = resolve_backend(Some(explicit_backend), None).expect("backend");
    assert_eq!(backend, explicit_backend);

    match prev {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[test]
fn test_resolve_backend_reads_env() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("CZAR_DB_BACKEND").ok();
    let expected_backend = compiled_backends()
        .first()
        .copied()
        .expect("compiled backend");
    unsafe {
        std::env::set_var("CZAR_DB_BACKEND", expected_backend.as_str());
    }

    let backend = resolve_backend(None, None).expect("backend");
    assert_eq!(backend, expected_backend);

    match prev {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[test]
fn test_resolve_backend_rejects_invalid_env_value() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("CZAR_DB_BACKEND").ok();
    unsafe {
        std::env::set_var("CZAR_DB_BACKEND", "mysql");
    }

    let err = resolve_backend(None, None).expect_err("invalid env backend should fail");
    assert!(err.to_string().contains("invalid CZAR_DB_BACKEND value"));

    match prev {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[test]
fn test_resolve_backend_defaults_to_only_compiled_backend() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("CZAR_DB_BACKEND").ok();
    unsafe {
        std::env::remove_var("CZAR_DB_BACKEND");
    }

    let compiled = compiled_backends();
    if compiled.len() == 1 {
        let backend = resolve_backend(None, None).expect("backend");
        assert_eq!(backend, compiled[0]);
    }

    match prev {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[test]
fn test_load_project_env_from_reads_env_file_from_ancestor_czar_yaml() {
    let _guard = env_lock().lock().expect("env lock");
    let prev_database = std::env::var("DATABASE_URL").ok();
    let prev_backend = std::env::var("CZAR_DB_BACKEND").ok();
    unsafe {
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("CZAR_DB_BACKEND");
    }

    let root = std::env::temp_dir().join(format!("czar-env-file-{}", Uuid::new_v4()));
    let child = root.join("nested").join("deeper");
    std::fs::create_dir_all(&child).expect("create nested dir");
    std::fs::write(root.join("czar.yaml"), "env_file: .env\n").expect("write czar.yaml");
    std::fs::write(
        root.join(".env"),
        "DATABASE_URL=sqlite:///tmp/from-env-file.db\nCZAR_DB_BACKEND=sqlite\n",
    )
    .expect("write .env");

    let loaded = load_project_env_from(&child).expect("load env file");
    assert_eq!(loaded, Some(root.join(".env")));
    assert_eq!(
        std::env::var("DATABASE_URL").ok().as_deref(),
        Some("sqlite:///tmp/from-env-file.db")
    );
    assert_eq!(
        std::env::var("CZAR_DB_BACKEND").ok().as_deref(),
        Some("sqlite")
    );

    let _ = std::fs::remove_dir_all(&root);
    match prev_database {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
    match prev_backend {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[test]
fn test_load_project_env_from_does_not_override_existing_env_vars() {
    let _guard = env_lock().lock().expect("env lock");
    let prev_database = std::env::var("DATABASE_URL").ok();
    unsafe {
        std::env::set_var("DATABASE_URL", "sqlite:///tmp/already-set.db");
    }

    let root = std::env::temp_dir().join(format!("czar-env-file-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create dir");
    std::fs::write(root.join("czar.yaml"), "env_file: .env\n").expect("write czar.yaml");
    std::fs::write(
        root.join(".env"),
        "DATABASE_URL=sqlite:///tmp/from-env-file.db\n",
    )
    .expect("write .env");

    load_project_env_from(&root).expect("load env file");
    assert_eq!(
        std::env::var("DATABASE_URL").ok().as_deref(),
        Some("sqlite:///tmp/already-set.db")
    );

    let _ = std::fs::remove_dir_all(&root);
    match prev_database {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
}

#[test]
fn test_project_contract_validation_enabled_from_defaults_true() {
    let root = std::env::temp_dir().join(format!("czar-validation-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create dir");

    let enabled =
        crate::bootstrap::project_contract_validation_enabled_from(&root).expect("validation");
    assert!(enabled);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_project_contract_validation_enabled_from_reads_czar_yaml() {
    let root = std::env::temp_dir().join(format!("czar-validation-{}", Uuid::new_v4()));
    let child = root.join("nested");
    std::fs::create_dir_all(&child).expect("create nested dir");
    std::fs::write(root.join("czar.yaml"), "validation:\n  enabled: false\n")
        .expect("write czar.yaml");

    let enabled =
        crate::bootstrap::project_contract_validation_enabled_from(&child).expect("validation");
    assert!(!enabled);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_project_notifications_from_reads_czar_yaml() {
    let root = std::env::temp_dir().join(format!("czar-notify-{}", Uuid::new_v4()));
    let child = root.join("nested");
    std::fs::create_dir_all(&child).expect("create nested dir");
    std::fs::write(
        root.join("czar.yaml"),
        "notifications:\n  on_failure:\n    email:\n      to: [team@example.com]\n    slack:\n      webhook_url: https://hooks.example.test/slack\n",
    )
    .expect("write czar.yaml");

    let notifications =
        crate::bootstrap::project_notifications_from(&child).expect("notifications");
    let on_failure = notifications.on_failure.expect("on_failure notifications");
    assert_eq!(
        on_failure.email.expect("email").to,
        vec!["team@example.com".to_string()]
    );
    assert_eq!(
        on_failure.slack.expect("slack").webhook_url,
        "https://hooks.example.test/slack"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(feature = "d1")]
#[test]
fn test_resolve_d1_config_reads_cli_and_env_values() {
    let _guard = env_lock().lock().expect("env lock");
    let prev_account = std::env::var("CZAR_D1_ACCOUNT_ID").ok();
    let prev_database = std::env::var("CZAR_D1_DATABASE_ID").ok();
    let prev_token = std::env::var("CZAR_D1_API_TOKEN").ok();
    let prev_base = std::env::var("CZAR_D1_BASE_URL").ok();
    unsafe {
        std::env::set_var("CZAR_D1_ACCOUNT_ID", "env-account");
        std::env::set_var("CZAR_D1_DATABASE_ID", "env-database");
        std::env::set_var("CZAR_D1_API_TOKEN", "env-token");
        std::env::set_var("CZAR_D1_BASE_URL", "https://env-base.invalid");
    }

    let args = DatabaseArgs {
        db_backend: Some(DatabaseBackend::D1),
        database_url: None,
        d1_account_id: Some("cli-account".to_string()),
        d1_database_id: None,
        d1_api_token: None,
        d1_base_url: None,
    };

    let config = crate::bootstrap::resolve_d1_config(&args).expect("d1 config");
    assert_eq!(config.account_id, "cli-account");
    assert_eq!(config.database_id, "env-database");
    assert_eq!(config.api_token, "env-token");
    assert_eq!(config.base_url.as_deref(), Some("https://env-base.invalid"));

    match prev_account {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_ACCOUNT_ID", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_ACCOUNT_ID") },
    }
    match prev_database {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_DATABASE_ID", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_DATABASE_ID") },
    }
    match prev_token {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_API_TOKEN", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_API_TOKEN") },
    }
    match prev_base {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_BASE_URL", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_BASE_URL") },
    }
}

#[test]
fn test_resolve_backend_picks_by_url_when_several_are_compiled() {
    let compiled = compiled_backends();
    if !(compiled.contains(&DatabaseBackend::Sqlite)
        && compiled.contains(&DatabaseBackend::Postgres))
    {
        return;
    }
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("CZAR_DB_BACKEND").ok();
    unsafe {
        std::env::remove_var("CZAR_DB_BACKEND");
    }
    let pick = |url: Option<&str>| resolve_backend(None, url).expect("backend");
    assert_eq!(
        pick(Some("postgres://localhost/czar")),
        DatabaseBackend::Postgres
    );
    assert_eq!(pick(Some("sqlite://./czar.db")), DatabaseBackend::Sqlite);
    match prev {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[test]
fn test_only_write_commands_run_migrations() {
    let needs = |args: &[&str]| {
        Cli::try_parse_from(args)
            .expect("parse")
            .command
            .needs_migrations()
    };
    assert!(needs(&["czar", "run", "start", "wf"]));
    assert!(needs(&["czar", "trigger", "wf"]));
    assert!(!needs(&["czar", "status"]));
    assert!(!needs(&["czar", "list-workflows"]));
}
