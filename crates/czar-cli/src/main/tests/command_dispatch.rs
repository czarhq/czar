use super::*;
use czar_core::database::RunRepository;
use uuid::Uuid;

#[tokio::test]
async fn test_handle_api_command_for_non_api_command() {
    let command = Commands::Init(Init);
    let handled = handle_api_command(command)
        .await
        .expect("handle non-api command");
    assert!(matches!(handled, Some(Commands::Init(_))));
}

#[tokio::test]
async fn test_handle_api_command_executes_validate_workflow() {
    let path = std::env::temp_dir().join(format!("czar-main-validate-{}.yaml", Uuid::new_v4()));
    std::fs::write(
        &path,
        r#"
name: wf-validate-main
tasks:
  only_task:
    executor: process
    command: "echo hi"
    output_type: str
"#,
    )
    .expect("write validation yaml");

    let command = Commands::ValidateWorkflow(ValidateWorkflow {
        file: path.to_string_lossy().to_string(),
    });
    let handled = handle_api_command(command)
        .await
        .expect("validate command should succeed");
    let _ = std::fs::remove_file(path);
    assert!(handled.is_none(), "validate command should return early");
}

#[tokio::test]
async fn test_dispatch_command_migrates_fresh_database() {
    let db = Arc::new(
        czar_store::SqliteDatabase::new(":memory:")
            .await
            .expect("create sqlite db"),
    );
    let yaml_path = std::env::temp_dir().join(format!("czar-main-fresh-{}.yaml", Uuid::new_v4()));
    std::fs::write(
        &yaml_path,
        "name: wf-fresh\ntasks:\n  step_a:\n    executor: process\n    command: \"echo a\"\n",
    )
    .expect("write yaml");

    let loaded = dispatch_command(
        Commands::CreateWorkflowYaml(CreateWorkflowYaml {
            file: yaml_path.to_string_lossy().to_string(),
            project: "local".to_string(),
            region: "local".to_string(),
        }),
        db.clone(),
    )
    .await;
    let _ = std::fs::remove_file(&yaml_path);

    loaded.expect("load on a fresh database");
}

#[tokio::test]
async fn test_dispatch_command_variants() {
    let db = setup_db().await;

    dispatch_command(Commands::Init(Init), db.clone())
        .await
        .expect("init");

    let yaml_path = std::env::temp_dir().join(format!("czar-main-{}.yaml", Uuid::new_v4()));
    std::fs::write(
        &yaml_path,
        r#"
name: wf-main
tasks:
  step_a:
    executor: process
    command: "echo a"
"#,
    )
    .expect("write yaml");
    dispatch_command(
        Commands::CreateWorkflowYaml(CreateWorkflowYaml {
            file: yaml_path.to_string_lossy().to_string(),
            project: "local".to_string(),
            region: "local".to_string(),
        }),
        db.clone(),
    )
    .await
    .expect("create workflow yaml");

    dispatch_command(Commands::ListWorkflows(ListWorkflows), db.clone())
        .await
        .expect("list workflows");

    dispatch_command(
        Commands::Trigger(Trigger {
            workflow_name: "wf-main".to_string(),
            selection: Default::default(),
        }),
        db.clone(),
    )
    .await
    .expect("trigger");

    dispatch_command(
        Commands::Status(Status {
            run_id: None,
            workflow: Some("wf-main".to_string()),
        }),
        db.clone(),
    )
    .await
    .expect("status");

    let run = db
        .list_runs(None)
        .await
        .expect("list runs")
        .into_iter()
        .next()
        .expect("run exists");
    dispatch_command(
        Commands::Tasks(Tasks {
            run_id: run.id.to_string(),
        }),
        db.clone(),
    )
    .await
    .expect("tasks");

    dispatch_command(
        Commands::Logs(Logs {
            run_id: run.id.to_string(),
            task: None,
            follow: false,
            poll_interval_ms: 1000,
        }),
        db.clone(),
    )
    .await
    .expect("logs");

    dispatch_command(
        Commands::DeleteWorkflow(DeleteWorkflow {
            workflow_name: "wf-main".to_string(),
        }),
        db.clone(),
    )
    .await
    .expect("delete workflow");

    let _ = std::fs::remove_file(yaml_path);
}

#[tokio::test]
async fn test_dispatch_command_run_error_path() {
    let db = setup_db().await;

    let run_err = dispatch_command(
        Commands::Run(Run {
            file: Some("/tmp/does-not-exist-workflow.yaml".to_string()),
            ..Default::default()
        }),
        db.clone(),
    )
    .await
    .expect_err("missing workflow file should error");
    assert!(
        run_err
            .to_string()
            .contains("Failed to load workflow source")
    );
}

#[tokio::test]
async fn test_run_cli_db_connect_error_path() {
    let cli = Cli {
        command: Commands::Init(Init),
        database: DatabaseArgs {
            db_backend: Some(DatabaseBackend::Postgres),
            database_url: Some("postgres://postgres:postgres@127.0.0.1:1/czar".to_string()),
            d1_account_id: None,
            d1_database_id: None,
            d1_api_token: None,
            d1_base_url: None,
        },
    };

    let err = run_cli(cli).await.expect_err("db connect should fail");
    assert!(
        err.to_string().contains("Connection")
            || err.to_string().contains("connection")
            || err.to_string().contains("refused")
            || err.to_string().contains("unable to open database file")
    );
}
