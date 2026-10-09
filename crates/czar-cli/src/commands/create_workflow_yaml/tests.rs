use super::*;
use czar_core::database::WorkflowRepository;
use czar_store::SqliteDatabase;

#[tokio::test]
async fn test_create_workflow_from_yaml_str_creates_workflow_and_tasks() {
    let db = SqliteDatabase::new(":memory:").await.expect("create db");
    db.run_migrations().await.expect("migrate");

    let yaml = r#"
name: wf-yaml
tasks:
  first:
    executor: process
    command: "echo first"
    output_type:
      data: str
  second:
    executor: process
    command: "echo second"
    depends_on: ["first"]
    input_type:
      first_data: str
    output_type:
      done: bool
    inputs:
      first_data:
        ref: tasks.first.output.data
"#;

    let workflow = create_workflow_from_yaml_str(&db, yaml, "/tmp/workflow.yaml", "local", "local")
        .await
        .expect("create workflow from yaml");
    assert_eq!(workflow.name, "wf-yaml");

    let tasks = db
        .list_workflow_tasks(workflow.id)
        .await
        .expect("list workflow tasks");
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[1].depends_on, vec!["first".to_string()]);
}

#[tokio::test]
async fn test_create_workflow_from_yaml_uses_cwd_when_file_path_has_no_parent() {
    let db = SqliteDatabase::new(":memory:").await.expect("create db");
    db.run_migrations().await.expect("migrate");

    let yaml = r#"
name: wf-yaml-no-parent
tasks:
  first:
    executor: process
    command: "echo first"
"#;

    let workflow = create_workflow_from_yaml_str(&db, yaml, "/", "local", "local")
        .await
        .expect("create workflow from yaml");
    assert_eq!(workflow.name, "wf-yaml-no-parent");
}

#[tokio::test]
async fn test_create_workflow_yaml_command_execute() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let path = std::env::temp_dir().join(format!(
        "czar-create-workflow-yaml-{}.yaml",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(
        &path,
        r#"
name: wf-yaml-cmd
tasks:
  first:
    executor: process
    command: "echo first"
"#,
    )
    .expect("write temp workflow");

    CreateWorkflowYaml {
        file: path.to_string_lossy().to_string(),
        project: "local".to_string(),
        region: "local".to_string(),
    }
    .execute(db.clone())
    .await
    .expect("command execute should succeed");

    let workflow = db
        .get_workflow("wf-yaml-cmd")
        .await
        .expect("workflow should exist");
    let tasks = db
        .list_workflow_tasks(workflow.id)
        .await
        .expect("list tasks");
    assert_eq!(tasks.len(), 1);

    let _ = std::fs::remove_file(path);
}
