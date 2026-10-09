use super::*;
use czar_core::database::{RunRepository, WorkflowRepository};
use czar_store::SqliteDatabase;

async fn setup_db() -> (Arc<SqliteDatabase>, uuid::Uuid) {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    let workflow = db
        .create_workflow(
            "wf-status",
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
    let run = db
        .create_run(workflow.id, "test")
        .await
        .expect("create run");
    (db, run.id)
}

#[tokio::test]
async fn test_status_command_branches() {
    let (db, run_id) = setup_db().await;

    Status {
        run_id: Some(run_id.to_string()),
        workflow: None,
    }
    .execute(db.clone())
    .await
    .expect("status by run id should succeed");

    Status {
        run_id: None,
        workflow: Some("wf-status".to_string()),
    }
    .execute(db.clone())
    .await
    .expect("status by workflow should succeed");

    Status {
        run_id: None,
        workflow: None,
    }
    .execute(db)
    .await
    .expect("status all should succeed");
}

#[tokio::test]
async fn test_status_command_prints_no_runs_for_empty_db() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    Status {
        run_id: None,
        workflow: None,
    }
    .execute(db)
    .await
    .expect("status with no runs should succeed");
}
