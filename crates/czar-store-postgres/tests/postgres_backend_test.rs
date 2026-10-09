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

async fn create_workflow_and_run(db: &PostgresDatabase, name: &str) -> Result<(Uuid, Uuid)> {
    let wf = db
        .create_workflow(
            name,
            Some("postgres coverage"),
            "dag",
            "local",
            "local",
            "dag",
            Some(serde_json::json!({"task_count": 2})),
            Some("*/5 * * * *"),
        )
        .await?;
    let run = db.create_run(wf.id, "test").await?;
    Ok((wf.id, run.id))
}

#[tokio::test]
async fn test_postgres_workflow_run_task_contract() -> Result<()> {
    let Some(ctx) = PostgresTestContext::setup().await? else {
        return Ok(());
    };
    let db = &ctx.db;

    let (workflow_id, run_id) = create_workflow_and_run(db, "pg-contract").await?;

    let wf = db.get_workflow("pg-contract").await?;
    assert_eq!(wf.id, workflow_id);
    assert_eq!(wf.schedule.as_deref(), Some("*/5 * * * *"));

    let all = db.list_workflows().await?;
    assert!(all.iter().any(|w| w.id == workflow_id));
    let by_id = db.get_workflow_by_id(workflow_id).await?;
    assert_eq!(by_id.name, "pg-contract");
    let by_ids = db.get_workflows_by_ids(&[workflow_id]).await?;
    assert_eq!(by_ids.len(), 1);

    db.create_workflow_tasks(
        workflow_id,
        &[
            NewWorkflowTask {
                task_index: 0,
                task_name: "a".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo a"}),
                signature: Some(serde_json::json!({"input":"str"})),
            },
            NewWorkflowTask {
                task_index: 1,
                task_name: "b".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["a".to_string()],
                params: serde_json::json!({"command":"echo b"}),
                signature: None,
            },
        ],
    )
    .await?;
    let workflow_tasks = db.list_workflow_tasks(workflow_id).await?;
    assert_eq!(workflow_tasks.len(), 2);

    db.batch_create_tasks(run_id, 2, "pg-contract", "process")
        .await?;
    let mut run_tasks = db.list_tasks(run_id).await?;
    assert_eq!(run_tasks.len(), 2);
    run_tasks.sort_by_key(|t| t.task_index);

    let first_task = run_tasks[0].id;
    let second_task = run_tasks[1].id;

    db.update_run_status(run_id, czar_core::models::RunStatus::Running, None)
        .await?;
    db.update_task_status(
        first_task,
        czar_core::models::TaskStatus::Dispatched,
        Some("exec-1"),
        None,
    )
    .await?;
    db.update_task_status(
        first_task,
        czar_core::models::TaskStatus::Running,
        Some("exec-1"),
        None,
    )
    .await?;
    db.append_task_log(first_task, "line-1\n").await?;
    db.update_task_output(first_task, serde_json::json!({"ok": true}))
        .await?;
    db.update_task_status(
        first_task,
        czar_core::models::TaskStatus::Success,
        Some("exec-1"),
        None,
    )
    .await?;

    let outputs = db
        .get_task_outputs(run_id, &[String::from("task_0")])
        .await?;
    assert!(outputs.contains_key("task_0"));
    let retry_meta = db.get_task_retry_meta(&[first_task, second_task]).await?;
    assert_eq!(retry_meta.len(), 2);
    assert_eq!(retry_meta[&first_task].attempts, 0);
    assert_eq!(retry_meta[&first_task].max_retries, 0);
    assert_eq!(retry_meta[&first_task].retry_delay_seconds, None);
    assert_eq!(db.get_task_run_id(first_task).await?, run_id);
    let identity = db.get_task_identity(first_task).await?;
    assert_eq!(identity.0, run_id);
    assert_eq!(identity.1, "task_0");

    let pending = db.get_pending_tasks().await?;
    assert!(pending.iter().any(|t| t.id == second_task));
    let pending_with_wf = db.get_pending_tasks_with_workflow(50).await?;
    assert!(pending_with_wf.iter().any(|t| t.task_id == second_task));

    db.batch_update_task_status(&[(
        second_task,
        czar_core::models::TaskStatus::Failed,
        Some("exec-2"),
        Some("batch failure"),
    )])
    .await?;
    let stats = db.get_run_task_stats(run_id).await?;
    assert_eq!(stats.0, 2);
    assert_eq!(stats.1, 2);
    assert_eq!(stats.2, 1);

    let retry_at = Utc::now() + Duration::seconds(1);
    db.reset_task_for_retry(second_task, Some("retry me"), Some(retry_at))
        .await?;
    let pending_again = db.get_pending_tasks().await?;
    assert!(pending_again.iter().any(|t| t.id == second_task));

    db.update_workflow_schedule(workflow_id, Some("*/5 * * * *"), true)
        .await?;
    let due_before = db.get_due_scheduled_workflows().await?;
    assert!(due_before.iter().any(|w| w.id == workflow_id));
    db.update_workflow_schedule_times(
        workflow_id,
        Utc::now(),
        Some(Utc::now() + Duration::hours(1)),
    )
    .await?;
    db.update_workflow_schedule(workflow_id, Some("0 * * * *"), true)
        .await?;
    let due_after = db.get_due_scheduled_workflows().await?;
    assert!(!due_after.iter().any(|w| w.id == workflow_id));

    db.cancel_run(run_id).await?;
    let run = db.get_run(run_id).await?;
    assert_eq!(run.status_str(), "cancelled");
    let listed_runs = db.list_runs(Some(workflow_id)).await?;
    assert!(!listed_runs.is_empty());
    let pending_runs = db.get_pending_runs().await?;
    assert!(pending_runs.iter().all(|r| r.id != run_id));

    db.delete_workflow("pg-contract").await?;
    assert!(db.get_workflow("pg-contract").await.is_err());

    ctx.cleanup().await
}

#[tokio::test]
async fn test_postgres_dag_dependency_and_deferred_jobs_contract() -> Result<()> {
    let Some(ctx) = PostgresTestContext::setup().await? else {
        return Ok(());
    };
    let db = &ctx.db;

    let (workflow_id, run_id) = create_workflow_and_run(db, "pg-deferrables").await?;

    db.batch_create_dag_tasks(
        run_id,
        &[
            NewTask {
                task_index: 0,
                task_name: "extract".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo extract"}),
                max_retries: 1,
                timeout_seconds: Some(60),
            },
            NewTask {
                task_index: 1,
                task_name: "load".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: serde_json::json!({"command":"echo load"}),
                max_retries: 0,
                timeout_seconds: Some(60),
            },
        ],
    )
    .await?;

    db.update_run_status(run_id, czar_core::models::RunStatus::Running, None)
        .await?;

    let tasks = db.list_tasks(run_id).await?;
    let extract_id = tasks
        .iter()
        .find(|t| t.task_name == "extract")
        .expect("extract task")
        .id;
    let load_id = tasks
        .iter()
        .find(|t| t.task_name == "load")
        .expect("load task")
        .id;

    let failed = db
        .mark_tasks_failed_by_dependency(run_id, &[String::from("extract")], "extract failed")
        .await?;
    assert_eq!(failed, vec![String::from("load")]);

    let running_before = db.get_running_tasks().await?;
    assert!(running_before.is_empty());
    db.update_task_status(
        extract_id,
        czar_core::models::TaskStatus::Running,
        Some("exec-3"),
        None,
    )
    .await?;
    let running_after = db.get_running_tasks().await?;
    assert!(running_after.iter().any(|t| t.id == extract_id));

    let job = db.create_deferred_job(
        extract_id,
        "custom_http",
        "job-1",
        serde_json::json!({"url":"http://example/status","completion_field":"state","completion_value":"done"}),
    )
    .await?;
    assert_eq!(job.status_str(), "pending");

    let pending_jobs = db.get_pending_deferred_jobs().await?;
    assert!(pending_jobs.iter().any(|j| j.id == job.id));
    db.update_deferred_job_status(job.id, czar_core::models::DeferredJobStatus::Polling, None)
        .await?;
    db.update_deferred_job_polled(job.id).await?;
    let by_task = db.get_deferred_jobs_for_task(extract_id).await?;
    assert_eq!(by_task.len(), 1);
    assert_eq!(by_task[0].status_str(), "polling");

    db.complete_deferred_job(job.id).await?;
    let completed = db.get_deferred_jobs_for_task(extract_id).await?;
    assert_eq!(completed[0].status_str(), "completed");

    let failed_job = db
        .create_deferred_job(
            load_id,
            "custom_http",
            "job-2",
            serde_json::json!({"url":"http://example/status"}),
        )
        .await?;
    db.fail_deferred_job(failed_job.id, "bad gateway").await?;

    let cancel_job = db
        .create_deferred_job(
            load_id,
            "custom_http",
            "job-3",
            serde_json::json!({"url":"http://example/status"}),
        )
        .await?;
    db.update_deferred_job_status(
        cancel_job.id,
        czar_core::models::DeferredJobStatus::Polling,
        None,
    )
    .await?;
    db.cancel_deferred_jobs_for_task(load_id).await?;
    let load_jobs = db.get_deferred_jobs_for_task(load_id).await?;
    assert!(load_jobs.iter().any(|j| j.status_str() == "failed"));
    assert!(load_jobs.iter().any(|j| j.status_str() == "cancelled"));

    db.delete_workflow("pg-deferrables").await?;
    assert!(db.get_workflow_by_id(workflow_id).await.is_err());

    ctx.cleanup().await
}
