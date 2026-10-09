use super::support::*;
use anyhow::Result;
use czar_core::database::{RunRepository, TaskRepository, WorkflowRepository};
use czar_core::scheduler::Scheduler;
use std::sync::Arc;
use tokio::time::{Duration, sleep, timeout};

#[tokio::test]
async fn test_scheduler_non_dag_pending_run_creates_tasks() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let workflow = db
        .create_workflow(
            "legacy-pending-run",
            None,
            "job",
            "local",
            "local",
            "process",
            Some(serde_json::json!({"task_count": 2})),
            None,
        )
        .await?;
    let run = db.create_run(workflow.id, "manual").await?;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            if db.list_tasks(run.id).await?.len() >= 2 {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(
        wait.is_ok(),
        "timed out waiting for scheduler to create tasks"
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_dag_pending_run_without_compiled_tasks_fails() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let workflow = db
        .create_workflow(
            "dag-missing-compiled-tasks",
            None,
            "dag",
            "local",
            "local",
            "dag",
            None,
            None,
        )
        .await?;
    let run = db.create_run(workflow.id, "manual").await?;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            let run_state = db.get_run(run.id).await?;
            if run_state.status_str() == "failed" {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(
        wait.is_ok(),
        "timed out waiting for missing-compiled-tasks failure"
    );
    let run_state = db.get_run(run.id).await?;
    assert_eq!(run_state.status_str(), "failed");
    assert!(
        run_state
            .error
            .as_deref()
            .unwrap_or("")
            .contains("missing compiled workflow_tasks")
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_dispatch_failure_marks_run_failed() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, _task_id) =
        create_running_run_with_task(db.as_ref(), "fail-run", 0, Some(10)).await;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysFail)),
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
    Ok(())
}

#[tokio::test]
async fn test_scheduler_failed_run_sends_builtin_slack_notification() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, _task_id) =
        create_running_run_with_task(db.as_ref(), "fail-run-notify", 0, Some(10)).await;

    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let captured = Arc::new(std::sync::Mutex::new(String::new()));
    let captured_clone = captured.clone();
    let handle_http = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let read = std::io::Read::read(&mut stream, &mut buf).expect("read request");
            *captured_clone.lock().expect("capture lock") =
                String::from_utf8_lossy(&buf[..read]).to_string();
            let response =
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: text/plain\r\n\r\nok";
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
            let _ = std::io::Write::flush(&mut stream);
        }
    });

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysFail)),
    });
    let mut config = test_config();
    config.notifications = slack_notifications(format!("http://{addr}/hooks/failure"));
    let scheduler = Scheduler::new_with_config(db.clone(), manager, config);
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            let run = db.get_run(run_id).await?;
            if run.status_str() == "failed" && !captured.lock().expect("capture lock").is_empty() {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(wait.is_ok(), "timed out waiting for failure notification");
    let request = captured.lock().expect("capture lock").clone();
    assert!(request.starts_with("POST /hooks/failure"));
    assert!(request.contains("fail-run-notify"));
    assert!(request.contains("task-a"));
    let _ = handle_http.join();
    Ok(())
}

#[tokio::test]
async fn test_scheduler_retry_then_success() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, _task_id) =
        create_running_run_with_task(db.as_ref(), "retry-then-success", 1, Some(10)).await;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::FailThenSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    wait_for_run_status(db.as_ref(), run_id, "success", 5).await?;
    handle.abort();

    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task");
    assert_eq!(task.status_str(), "success");
    assert!(task.attempts >= 1);
    Ok(())
}
