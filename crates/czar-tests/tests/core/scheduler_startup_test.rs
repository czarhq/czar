use std::sync::Arc;

use async_trait::async_trait;
use czar_core::config::OrchestratorConfig;
use czar_core::executor::{Executor, StatusUpdate};
use czar_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use czar_core::models::Workflow;
use czar_core::scheduler::Scheduler;
use czar_store::SqliteDatabase;
use sqlx::query;
use tokio::time::{Duration, sleep, timeout};
use uuid::Uuid;

#[derive(Default)]
struct NoopExecutor;

#[async_trait]
impl Executor for NoopExecutor {
    async fn execute(
        &self,
        _task_id: Uuid,
        _job_name: &str,
        _params: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        Ok("noop".to_string())
    }

    async fn set_status_channel(&self, _tx: tokio::sync::mpsc::UnboundedSender<StatusUpdate>) {}
}

#[derive(Default)]
struct NoopExecutorManager;

#[async_trait]
impl ExecutorManagerTrait for NoopExecutorManager {
    async fn get_executor(
        &self,
        _executor_type: &str,
        _workflow: &Workflow,
    ) -> anyhow::Result<Arc<dyn Executor>> {
        Ok(Arc::new(NoopExecutor))
    }
}

async fn setup_db() -> Arc<SqliteDatabase> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
    db.run_migrations().await.expect("migrations");
    db
}

fn init_tracing() -> tracing::dispatcher::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::set_default(subscriber)
}

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        poll_interval_secs: 0.01,
        max_tasks_per_batch: 10,
        max_concurrent_dispatches: 2,
        max_concurrent_status_checks: 2,
        db_pool_size: 2,
        enable_triggerer: false,
        notifications: czar_core::notifications::NotificationsConfig::default(),
        ..OrchestratorConfig::default()
    }
}

#[tokio::test]
async fn test_scheduler_idle_run_can_start_and_abort() {
    let _trace = init_tracing();
    let db = setup_db().await;
    let scheduler = Scheduler::new_with_config(db, Arc::new(NoopExecutorManager), test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    sleep(Duration::from_millis(50)).await;
    handle.abort();
    let result = handle.await;
    assert!(result.is_err(), "aborted scheduler task should error");
}

#[tokio::test]
async fn test_scheduler_new_constructor_uses_default_config() {
    let _trace = init_tracing();
    let db = setup_db().await;
    let _scheduler = Scheduler::new(db, Arc::new(NoopExecutorManager));
}

#[tokio::test]
async fn test_scheduler_start_triggerer_initialization_path() {
    let _trace = init_tracing();
    let db = setup_db().await;
    let mut config = test_config();
    config.enable_triggerer = true;

    let scheduler = Scheduler::new_with_config(db, Arc::new(NoopExecutorManager), config);
    let attempt = timeout(Duration::from_secs(5), scheduler.start_triggerer()).await;
    if let Ok(handle) = attempt {
        handle.abort();
    }
}

#[tokio::test]
async fn test_scheduler_run_tolerates_database_errors_in_loop() {
    let _trace = init_tracing();
    let db = setup_db().await;
    // Force query errors in the main loop to cover recovery branches.
    query("DROP TABLE tasks")
        .execute(db.pool())
        .await
        .expect("drop tasks");
    query("DROP TABLE runs")
        .execute(db.pool())
        .await
        .expect("drop runs");
    query("DROP TABLE workflows")
        .execute(db.pool())
        .await
        .expect("drop workflows");

    let scheduler = Scheduler::new_with_config(db, Arc::new(NoopExecutorManager), test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    sleep(Duration::from_millis(80)).await;
    handle.abort();
    let result = handle.await;
    assert!(result.is_err(), "aborted scheduler task should error");
}

#[tokio::test]
async fn test_scheduler_run_continues_when_triggerer_start_fails() {
    let _trace = init_tracing();
    let db = setup_db().await;
    let mut config = test_config();
    config.enable_triggerer = true;

    let prev_creds = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok();
    unsafe {
        std::env::set_var(
            "GOOGLE_APPLICATION_CREDENTIALS",
            "/tmp/czar-definitely-missing-creds.json",
        );
    }

    let scheduler = Scheduler::new_with_config(db, Arc::new(NoopExecutorManager), config);
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    sleep(Duration::from_millis(120)).await;
    assert!(
        !handle.is_finished(),
        "scheduler should keep looping even when triggerer startup fails"
    );
    handle.abort();

    match prev_creds {
        Some(v) => unsafe { std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS", v) },
        None => unsafe { std::env::remove_var("GOOGLE_APPLICATION_CREDENTIALS") },
    }
}
