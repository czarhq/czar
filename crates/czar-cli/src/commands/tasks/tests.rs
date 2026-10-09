use super::*;
use czar_core::database::{NewTask, RunRepository, TaskRepository, WorkflowRepository};
use czar_store::SqliteDatabase;

#[tokio::test]
async fn test_tasks_command_empty_and_non_empty() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    let workflow = db
        .create_workflow(
            "wf-tasks", None, "job", "local", "local", "process", None, None,
        )
        .await
        .expect("create workflow");
    let run = db
        .create_run(workflow.id, "test")
        .await
        .expect("create run");

    Tasks {
        run_id: run.id.to_string(),
    }
    .execute(db.clone())
    .await
    .expect("tasks command should handle empty task list");

    db.batch_create_dag_tasks(
        run.id,
        &[NewTask {
            task_index: 0,
            task_name: "task-a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo hi"}),
            max_retries: 0,
            timeout_seconds: Some(10),
        }],
    )
    .await
    .expect("create task");

    Tasks {
        run_id: run.id.to_string(),
    }
    .execute(db)
    .await
    .expect("tasks command should handle populated task list");
}
