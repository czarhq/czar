#[path = "support/postgres_docker.rs"]
mod postgres_docker;

use anyhow::Result;
use chrono::{Duration, Utc};
use czar_core::database::{
    NewTask, NewWorkflowTask, RunRepository, ScheduleRepository, TaskRepository, WorkflowRepository,
};
use czar_store_postgres::PostgresDatabase;
use uuid::Uuid;

struct PostgresTestContext {
    server: postgres_docker::PostgresServerHarness,
    admin: PostgresDatabase,
    db_name: String,
    db: PostgresDatabase,
}

impl PostgresTestContext {
    async fn setup() -> Result<Option<Self>> {
        let Some(server) = postgres_docker::PostgresServerHarness::setup().await? else {
            return Ok(None);
        };
        let admin = PostgresDatabase::new(&server.admin_url()?).await?;

        let db_name = format!("cov_{}", Uuid::new_v4().simple());
        let create_database_sql = format!(r#"CREATE DATABASE "{}""#, db_name);
        sqlx::query(sqlx::AssertSqlSafe(create_database_sql))
            .execute(admin.pool())
            .await?;

        let db_url = postgres_docker::with_database(server.base_url(), &db_name)?;
        let db = PostgresDatabase::new(&db_url).await?;
        db.run_migrations().await?;

        Ok(Some(Self {
            server,
            admin,
            db_name,
            db,
        }))
    }

    async fn cleanup(self) -> Result<()> {
        self.db.pool().close().await;
        sqlx::query(
            "SELECT pg_terminate_backend(pid)
             FROM pg_stat_activity
             WHERE datname = $1 AND pid <> pg_backend_pid()",
        )
        .bind(&self.db_name)
        .execute(self.admin.pool())
        .await?;
        let drop_database_sql = format!(r#"DROP DATABASE IF EXISTS "{}""#, self.db_name);
        sqlx::query(sqlx::AssertSqlSafe(drop_database_sql))
            .execute(self.admin.pool())
            .await?;
        self.server.cleanup().await?;
        Ok(())
    }
}

fn task_id(tasks: &[czar_core::models::Task], name: &str) -> Uuid {
    tasks
        .iter()
        .find(|t| t.task_name == name)
        .unwrap_or_else(|| panic!("missing task: {name}"))
        .id
}
#[tokio::test]
async fn test_postgres_branch_and_edge_paths() -> Result<()> {
    let Some(ctx) = PostgresTestContext::setup().await? else {
        return Ok(());
    };
    let db = &ctx.db;

    let wf = db
        .create_workflow(
            "pg-edges",
            Some("edge cases"),
            "job",
            "region",
            "project",
            "dag",
            None,
            None,
        )
        .await?;
    let workflow_id = wf.id;

    let by_ids_empty = db.get_workflows_by_ids(&[]).await?;
    assert!(by_ids_empty.is_empty());
    let due_initial = db.get_due_scheduled_workflows().await?;
    assert!(!due_initial.iter().any(|w| w.id == workflow_id));

    db.create_workflow_tasks(
        workflow_id,
        &[NewWorkflowTask {
            task_index: 0,
            task_name: "extract".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo extract"}),
            signature: None,
        }],
    )
    .await?;
    db.create_workflow_tasks(
        workflow_id,
        &[
            NewWorkflowTask {
                task_index: 0,
                task_name: "extract".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo extract"}),
                signature: Some(serde_json::json!({"output":"json"})),
            },
            NewWorkflowTask {
                task_index: 1,
                task_name: "load".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: serde_json::json!({"command":"echo load"}),
                signature: None,
            },
        ],
    )
    .await?;
    let workflow_tasks = db.list_workflow_tasks(workflow_id).await?;
    assert_eq!(workflow_tasks.len(), 2);

    let run = db.create_run(workflow_id, "tests").await?;
    let run_id = run.id;
    let pending_runs = db.get_pending_runs().await?;
    assert!(pending_runs.iter().any(|r| r.id == run_id));
    let runs_all = db.list_runs(None).await?;
    assert!(runs_all.iter().any(|r| r.id == run_id));

    db.batch_create_dag_tasks(
        run_id,
        &[
            NewTask {
                task_index: 0,
                task_name: "extract".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo extract"}),
                max_retries: 3,
                timeout_seconds: Some(60),
            },
            NewTask {
                task_index: 1,
                task_name: "load".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: serde_json::json!({"command":"echo load"}),
                max_retries: 3,
                timeout_seconds: Some(60),
            },
        ],
    )
    .await?;

    db.batch_update_task_status(&[]).await?;
    let no_failed = db
        .mark_tasks_failed_by_dependency(run_id, &[], "should not apply")
        .await?;
    assert!(no_failed.is_empty());

    let empty_outputs = db.get_task_outputs(run_id, &[]).await?;
    assert!(empty_outputs.is_empty());

    let tasks = db.list_tasks(run_id).await?;
    let extract_id = task_id(&tasks, "extract");
    let load_id = task_id(&tasks, "load");

    let empty_retry = db.get_task_retry_meta(&[]).await?;
    assert!(empty_retry.is_empty());

    let retry_at = Utc::now() + Duration::seconds(30);
    db.reset_task_for_retry(extract_id, Some("retry"), Some(retry_at))
        .await?;
    let mut tasks = db.list_tasks(run_id).await?;
    let extract_after_reset = tasks
        .iter()
        .find(|t| t.id == extract_id)
        .expect("extract after reset");
    assert!(extract_after_reset.retry_at.is_some());

    db.update_task_status(
        extract_id,
        czar_core::models::TaskStatus::Paused,
        None,
        Some("paused"),
    )
    .await?;
    tasks = db.list_tasks(run_id).await?;
    let extract_paused = tasks.iter().find(|t| t.id == extract_id).expect("paused");
    assert_eq!(extract_paused.status_str(), "paused");
    assert!(extract_paused.retry_at.is_some());

    db.update_task_status(
        extract_id,
        czar_core::models::TaskStatus::Running,
        Some("exec-edge"),
        None,
    )
    .await?;
    tasks = db.list_tasks(run_id).await?;
    let extract_running = tasks.iter().find(|t| t.id == extract_id).expect("running");
    assert_eq!(extract_running.status_str(), "running");
    assert!(extract_running.retry_at.is_none());
    assert!(extract_running.started_at.is_some());

    db.update_run_status(run_id, czar_core::models::RunStatus::Running, None)
        .await?;
    let run_running = db.get_run(run_id).await?;
    assert_eq!(run_running.status_str(), "running");
    assert!(run_running.started_at.is_some());
    assert!(run_running.finished_at.is_none());

    let pending_with_unsatisfied = db.get_pending_tasks_with_workflow(10).await?;
    assert!(
        pending_with_unsatisfied
            .iter()
            .all(|t| t.task_name != "load")
    );

    db.update_task_status(
        extract_id,
        czar_core::models::TaskStatus::Success,
        Some("exec-edge"),
        None,
    )
    .await?;
    let pending_with_satisfied = db.get_pending_tasks_with_workflow(10).await?;
    assert!(pending_with_satisfied.iter().any(|t| t.task_id == load_id));

    let run_missing_dep = db.create_run(workflow_id, "tests").await?;
    db.batch_create_dag_tasks(
        run_missing_dep.id,
        &[NewTask {
            task_index: 1,
            task_name: "load_only".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["extract".to_string()],
            params: serde_json::json!({"command":"echo load-only"}),
            max_retries: 0,
            timeout_seconds: None,
        }],
    )
    .await?;
    db.update_run_status(
        run_missing_dep.id,
        czar_core::models::RunStatus::Running,
        None,
    )
    .await?;
    let pending_with_missing_dep_task = db.get_pending_tasks_with_workflow(50).await?;
    assert!(
        pending_with_missing_dep_task
            .iter()
            .any(|t| t.run_id == run_missing_dep.id && t.task_name == "load_only")
    );

    let deferred = db
        .create_deferred_job(
            extract_id,
            "custom_http",
            "edge-job-1",
            serde_json::json!({"url":"http://example/status"}),
        )
        .await?;
    assert!(deferred.started_at.is_none());
    db.update_deferred_job_status(
        deferred.id,
        czar_core::models::DeferredJobStatus::Polling,
        None,
    )
    .await?;
    let deferred_polled = db.get_deferred_jobs_for_task(extract_id).await?;
    assert!(
        deferred_polled
            .iter()
            .any(|j| j.id == deferred.id && j.started_at.is_some())
    );

    let deferred_completed = db
        .create_deferred_job(
            extract_id,
            "custom_http",
            "edge-job-2",
            serde_json::json!({"url":"http://example/status"}),
        )
        .await?;
    db.complete_deferred_job(deferred_completed.id).await?;
    let deferred_failed = db
        .create_deferred_job(
            extract_id,
            "custom_http",
            "edge-job-3",
            serde_json::json!({"url":"http://example/status"}),
        )
        .await?;
    db.fail_deferred_job(deferred_failed.id, "failed").await?;

    let deferred_cancelled = db
        .create_deferred_job(
            extract_id,
            "custom_http",
            "edge-job-4",
            serde_json::json!({"url":"http://example/status"}),
        )
        .await?;
    db.cancel_deferred_jobs_for_task(extract_id).await?;
    let deferred_after = db.get_deferred_jobs_for_task(extract_id).await?;
    assert!(deferred_after.iter().any(|j| j.id == deferred_completed.id
        && j.started_at.is_some()
        && j.finished_at.is_some()));
    assert!(
        deferred_after.iter().any(|j| j.id == deferred_failed.id
            && j.started_at.is_some()
            && j.finished_at.is_some())
    );
    assert!(
        deferred_after
            .iter()
            .any(|j| j.id == deferred_cancelled.id && j.status_str() == "cancelled")
    );

    let pending_deferred = db.get_pending_deferred_jobs().await?;
    assert!(pending_deferred.iter().all(|j| j.task_id != extract_id));

    db.update_run_status(
        run_id,
        czar_core::models::RunStatus::Failed,
        Some("edge fail"),
    )
    .await?;
    let run_failed = db.get_run(run_id).await?;
    assert_eq!(run_failed.status_str(), "failed");
    assert!(run_failed.finished_at.is_some());
    assert_eq!(run_failed.error.as_deref(), Some("edge fail"));

    db.delete_workflow("pg-edges").await?;
    assert!(db.get_workflow("pg-edges").await.is_err());

    ctx.cleanup().await
}
