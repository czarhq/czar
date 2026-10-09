use super::support::*;
use anyhow::Result;
use czar_core::database::TaskRepository;
use czar_core::scheduler::Scheduler;
use std::sync::Arc;

#[tokio::test]
async fn test_scheduler_output_validation_rejects_mismatched_success_output() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let run_id = create_pending_run_with_workflow_task_signature(
        db.as_ref(),
        "output-validation-mismatch",
        serde_json::json!({
            "output_type": "int"
        }),
    )
    .await?;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    wait_for_run_status(db.as_ref(), run_id, "failed", 2).await?;
    handle.abort();

    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task");
    assert_eq!(task.status_str(), "failed");
    assert!(
        task.error
            .as_deref()
            .unwrap_or_default()
            .contains("output validation failed")
    );
    assert!(
        task.output.is_none(),
        "invalid output should not be persisted"
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_output_validation_rejects_missing_success_output() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let run_id = create_pending_run_with_workflow_task_signature(
        db.as_ref(),
        "output-validation-missing",
        serde_json::json!({
            "output_type": {"ok": "bool"}
        }),
    )
    .await?;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::SuccessWithoutOutput)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    wait_for_run_status(db.as_ref(), run_id, "failed", 2).await?;
    handle.abort();

    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task");
    assert_eq!(task.status_str(), "failed");
    assert!(
        task.error
            .as_deref()
            .unwrap_or_default()
            .contains("returned no output")
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_output_validation_allows_matching_success_output() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let run_id = create_pending_run_with_workflow_task_signature(
        db.as_ref(),
        "output-validation-match",
        serde_json::json!({
            "output_type": {"ok": "bool"}
        }),
    )
    .await?;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    wait_for_run_status(db.as_ref(), run_id, "success", 2).await?;
    handle.abort();

    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task");
    assert_eq!(task.status_str(), "success");
    assert_eq!(
        task.output.expect("output").0,
        serde_json::json!({"ok": true})
    );
    Ok(())
}
