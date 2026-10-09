mod support;

use anyhow::{Context, Result};
use czar_core::database::{
    NewTask, RunRepository, TaskRepository, WorkflowRepository, WorkflowSnapshotRepository,
};
use czar_core::models::{RunStatus, TaskStatus};
use czar_store_d1::{D1Config, D1Database};

use support::{
    TEST_ACCOUNT_ID, TEST_DATABASE_ID, TEST_TOKEN, harness_dir, reserve_local_port,
    start_wrangler_dev, wait_for_http_status,
};

#[tokio::test]
#[ignore = "requires local wrangler dev harness; run `just test-cloudflare-d1-local`"]
async fn wrangler_dev_d1_smoke() -> Result<()> {
    let port = reserve_local_port()?;
    let harness = start_wrangler_dev(&harness_dir(), "wrangler.jsonc", "127.0.0.1", port).await?;
    wait_for_http_status(
        &format!("{}/health", harness.base_url()),
        reqwest::StatusCode::OK,
        None,
    )
    .await?;

    let db = D1Database::new(&D1Config {
        account_id: TEST_ACCOUNT_ID.to_string(),
        database_id: TEST_DATABASE_ID.to_string(),
        api_token: TEST_TOKEN.to_string(),
        base_url: Some(harness.base_url()),
    })
    .await?;

    db.run_migrations().await?;

    let workflow = db
        .create_workflow(
            "cloudflare-local",
            Some("wrangler-backed smoke test"),
            "job-local",
            "wnam",
            "local-project",
            "process",
            None,
            None,
        )
        .await?;

    let snapshot = db
        .create_or_get_snapshot(
            workflow.id,
            "snapshot-local",
            serde_json::json!([{ "task_name": "extract" }, { "task_name": "transform" }]),
            Some("name: cloudflare-local".to_string()),
        )
        .await?;
    db.update_workflow_snapshot(workflow.id, snapshot.id)
        .await?;

    let run = db.create_run(workflow.id, "integration").await?;
    assert_eq!(run.snapshot_id, Some(snapshot.id));
    db.update_run_status(run.id, RunStatus::Running, None)
        .await?;

    db.batch_create_dag_tasks(
        run.id,
        &[
            NewTask {
                task_index: 0,
                task_name: "extract".to_string(),
                executor_type: "process".to_string(),
                depends_on: Vec::new(),
                params: serde_json::json!({ "step": "extract" }),
                max_retries: 1,
                timeout_seconds: Some(30),
            },
            NewTask {
                task_index: 1,
                task_name: "transform".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: serde_json::json!({ "step": "transform" }),
                max_retries: 1,
                timeout_seconds: Some(30),
            },
        ],
    )
    .await?;

    let pending_before = db.get_pending_tasks_with_workflow(10).await?;
    assert_eq!(pending_before.len(), 1);
    assert_eq!(pending_before[0].task_name, "extract");

    let extract = db
        .list_tasks(run.id)
        .await?
        .into_iter()
        .find(|task| task.task_name == "extract")
        .context("missing extract task")?;
    db.update_task_status(extract.id, TaskStatus::Running, Some("exec-extract"), None)
        .await?;
    db.update_task_output(extract.id, serde_json::json!({ "rows": 2 }))
        .await?;
    db.update_task_status(extract.id, TaskStatus::Success, Some("exec-extract"), None)
        .await?;

    let pending_after = db.get_pending_tasks_with_workflow(10).await?;
    assert_eq!(pending_after.len(), 1);
    assert_eq!(pending_after[0].task_name, "transform");

    let outputs = db
        .get_task_outputs(run.id, &["extract".to_string()])
        .await?;
    assert_eq!(
        outputs.get("extract"),
        Some(&serde_json::json!({ "rows": 2 }))
    );

    let stats = db.get_run_task_stats(run.id).await?;
    assert_eq!(stats.0, 2);

    Ok(())
}
