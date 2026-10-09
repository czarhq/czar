use super::*;
use czar_core::database::WorkflowRepository;
use czar_store::SqliteDatabase;

#[tokio::test]
async fn test_list_workflows_empty_and_non_empty() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    ListWorkflows
        .execute(db.clone())
        .await
        .expect("listing empty workflows should succeed");

    db.create_workflow("wf", None, "job", "local", "local", "process", None, None)
        .await
        .expect("create workflow");

    ListWorkflows
        .execute(db)
        .await
        .expect("listing workflows should succeed");
}
