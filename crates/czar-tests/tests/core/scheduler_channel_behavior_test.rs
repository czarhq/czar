use anyhow::Result;
use async_trait::async_trait;
use czar_core::config::OrchestratorConfig;
use czar_core::database::{NewTask, RunRepository, TaskRepository, WorkflowRepository};
use czar_core::executor::{Executor, StatusUpdate};
use czar_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use czar_core::models::Workflow;
use czar_core::scheduler::Scheduler;
use czar_core::triggerer::JobCompletionNotification;
use czar_store::SqliteDatabase;
use std::sync::Arc;
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

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig::fast_test()
}

fn init_tracing() -> tracing::dispatcher::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::set_default(subscriber)
}

async fn create_running_run_with_task(
    db: &SqliteDatabase,
    name: &str,
    timeout_seconds: Option<i32>,
) -> (Uuid, Uuid) {
    let workflow = db
        .create_workflow(name, None, "job", "local", "local", "process", None, None)
        .await
        .expect("workflow");
    let run = db.create_run(workflow.id, "test").await.expect("run");
    db.batch_create_dag_tasks(
        run.id,
        &[NewTask {
            task_index: 0,
            task_name: "task-a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo hi"}),
            max_retries: 0,
            timeout_seconds,
        }],
    )
    .await
    .expect("create task");
    db.update_run_status(run.id, czar_core::models::RunStatus::Running, None)
        .await
        .expect("run running");
    let task_id = db
        .list_tasks(run.id)
        .await
        .expect("list tasks")
        .into_iter()
        .next()
        .expect("task exists")
        .id;
    db.update_task_status(
        task_id,
        czar_core::models::TaskStatus::Running,
        Some("exec"),
        None,
    )
    .await
    .expect("task running");
    (run.id, task_id)
}

#[tokio::test]
async fn test_scheduler_processes_enqueued_job_completion_success_and_failure() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (success_run_id, success_task_id) =
        create_running_run_with_task(db.as_ref(), "completion-success", Some(10)).await;
    let (failure_run_id, failure_task_id) =
        create_running_run_with_task(db.as_ref(), "completion-failure", Some(10)).await;

    let scheduler = Arc::new(Scheduler::new_with_config(
        db.clone(),
        Arc::new(NoopExecutorManager),
        test_config(),
    ));
    let runner = Arc::clone(&scheduler);
    let handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    scheduler.enqueue_job_completion(JobCompletionNotification {
        task_id: success_task_id,
        success: true,
        error: None,
    });
    scheduler.enqueue_job_completion(JobCompletionNotification {
        task_id: failure_task_id,
        success: false,
        error: None,
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            let success_run = db.get_run(success_run_id).await?;
            let failure_run = db.get_run(failure_run_id).await?;
            if success_run.status_str() == "success" && failure_run.status_str() == "failed" {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(wait.is_ok(), "timed out waiting for completion events");
    let success_task = db
        .list_tasks(success_run_id)
        .await?
        .into_iter()
        .next()
        .expect("success task");
    assert_eq!(success_task.status_str(), "success");
    let failure_task = db
        .list_tasks(failure_run_id)
        .await?
        .into_iter()
        .next()
        .expect("failure task");
    assert_eq!(failure_task.status_str(), "failed");
    assert!(
        failure_task
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Deferred job failed")
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_keeps_running_when_enqueued_updates_hit_db_errors() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let scheduler = Arc::new(Scheduler::new_with_config(
        db.clone(),
        Arc::new(NoopExecutorManager),
        test_config(),
    ));
    let runner = Arc::clone(&scheduler);
    let handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    sqlx::query("DROP TABLE tasks").execute(db.pool()).await?;
    scheduler.enqueue_status_update(StatusUpdate {
        task_id: Uuid::new_v4(),
        status: "running".to_string(),
        log: Some("hello".to_string()),
        output: Some(serde_json::json!({"x": 1})),
        error: None,
        progress: None,
    });
    scheduler.enqueue_job_completion(JobCompletionNotification {
        task_id: Uuid::new_v4(),
        success: true,
        error: None,
    });
    scheduler.enqueue_job_completion(JobCompletionNotification {
        task_id: Uuid::new_v4(),
        success: false,
        error: Some("boom".to_string()),
    });

    sleep(Duration::from_millis(120)).await;
    assert!(
        !handle.is_finished(),
        "scheduler should keep looping after recoverable DB errors"
    );
    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_scheduler_does_not_timeout_tasks_with_non_positive_timeout() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, _task_id) =
        create_running_run_with_task(db.as_ref(), "timeout-zero", Some(0)).await;
    let scheduler = Arc::new(Scheduler::new_with_config(
        db.clone(),
        Arc::new(NoopExecutorManager),
        test_config(),
    ));
    let runner = Arc::clone(&scheduler);
    let handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    sleep(Duration::from_millis(120)).await;
    handle.abort();

    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task");
    assert_eq!(task.status_str(), "running");
    Ok(())
}

#[tokio::test]
async fn test_scheduler_keeps_running_when_deferred_job_creation_fails() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let scheduler = Arc::new(Scheduler::new_with_config(
        db,
        Arc::new(NoopExecutorManager),
        test_config(),
    ));
    let runner = Arc::clone(&scheduler);
    let handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    scheduler.enqueue_status_update(StatusUpdate {
        task_id: Uuid::new_v4(),
        status: "success".to_string(),
        log: None,
        output: Some(serde_json::json!({
            "deferred": [{
                "service_type": "custom_http",
                "job_id": "missing-task-job",
                "url": "http://example.com/status"
            }]
        })),
        error: None,
        progress: None,
    });

    sleep(Duration::from_millis(120)).await;
    assert!(
        !handle.is_finished(),
        "scheduler should continue after deferred-job creation errors"
    );
    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_scheduler_processes_enqueued_success_update_and_completes_run() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, task_id) =
        create_running_run_with_task(db.as_ref(), "status-update-success", Some(10)).await;
    let scheduler = Arc::new(Scheduler::new_with_config(
        db.clone(),
        Arc::new(NoopExecutorManager),
        test_config(),
    ));
    let runner = Arc::clone(&scheduler);
    let handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    scheduler.enqueue_status_update(StatusUpdate {
        task_id,
        status: "success".to_string(),
        log: Some("done".to_string()),
        output: Some(serde_json::json!({"ok": true})),
        error: None,
        progress: None,
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            let run = db.get_run(run_id).await?;
            if run.status_str() == "success" {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(wait.is_ok(), "timed out waiting for successful run update");
    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task exists");
    assert_eq!(task.status_str(), "success");
    assert!(task.logs.as_deref().unwrap_or_default().contains("done"));
    assert_eq!(
        task.output
            .as_ref()
            .map(czar_core::models::json_inner)
            .cloned(),
        Some(serde_json::json!({"ok": true}))
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_select_recv_error_path_when_idle() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    sqlx::query("DROP TABLE tasks").execute(db.pool()).await?;

    let mut config = test_config();
    config.poll_interval_secs = 5.0;

    let scheduler = Arc::new(Scheduler::new_with_config(
        db,
        Arc::new(NoopExecutorManager),
        config,
    ));
    let runner = Arc::clone(&scheduler);
    let handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    sleep(Duration::from_millis(80)).await;
    scheduler.enqueue_status_update(StatusUpdate {
        task_id: Uuid::new_v4(),
        status: "running".to_string(),
        log: Some("late-update".to_string()),
        output: None,
        error: None,
        progress: None,
    });

    sleep(Duration::from_millis(120)).await;
    assert!(
        !handle.is_finished(),
        "scheduler should survive status-update errors in idle receive path"
    );
    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_scheduler_select_recv_logs_error_for_terminal_update_when_idle() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    sqlx::query("DROP TABLE tasks").execute(db.pool()).await?;

    let mut config = test_config();
    config.poll_interval_secs = 5.0;

    let scheduler = Arc::new(Scheduler::new_with_config(
        db,
        Arc::new(NoopExecutorManager),
        config,
    ));
    let runner = Arc::clone(&scheduler);
    let handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    sleep(Duration::from_millis(80)).await;
    scheduler.enqueue_status_update(StatusUpdate {
        task_id: Uuid::new_v4(),
        status: "success".to_string(),
        log: None,
        output: None,
        error: None,
        progress: None,
    });

    sleep(Duration::from_millis(120)).await;
    assert!(
        !handle.is_finished(),
        "scheduler should survive terminal status-update errors in idle receive path"
    );
    handle.abort();
    Ok(())
}
