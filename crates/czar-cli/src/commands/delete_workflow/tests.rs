use super::*;
use czar_core::database::WorkflowRepository;
use czar_store::SqliteDatabase;

#[tokio::test]
async fn test_delete_workflow_command() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    db.create_workflow(
        "wf-delete",
        None,
        "job",
        "local",
        "local",
        "process",
        None,
        None,
    )
    .await
    .expect("create workflow");

    DeleteWorkflow {
        workflow_name: "wf-delete".to_string(),
    }
    .execute(db.clone())
    .await
    .expect("delete workflow should succeed");

    let workflows = db.list_workflows().await.expect("list workflows");
    assert!(workflows.is_empty());
}
