use super::*;

#[tokio::test]
async fn test_execute_process_reports_success_logs_and_output() {
    let executor = ProcessExecutor::new(None);
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(
            task_id,
            "printf 'line\\n'; printf 'CZAR_OUTPUT:{\"ok\":true}\\n'; printf 'warn\\n' 1>&2",
            Some(serde_json::json!({"task_name": "demo"})),
        )
        .await
        .expect("execute process");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    assert!(
        updates
            .iter()
            .any(|u| u.status == "running" && u.task_id == task_id)
    );
    assert!(updates.iter().any(|u| {
        u.status == "log" && {
            let log = u.log.as_deref().unwrap_or("");
            log.contains(" line") || log.contains("stdout: line")
        }
    }));
    assert!(updates.iter().any(|u| {
        u.status == "log" && {
            let log = u.log.as_deref().unwrap_or("");
            log.contains(" warn") || log.contains("stderr: warn")
        }
    }));
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    assert_eq!(terminal.output, Some(serde_json::json!({"ok": true})));
    assert!(terminal.error.is_none());

    let states = executor.process_states.read().await;
    let state = states.get(&execution_id).expect("process state");
    assert!(matches!(state, ProcessStatus::Success));
}

#[tokio::test]
async fn test_execute_process_reports_failed_status() {
    let executor = ProcessExecutor::new(None);
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(task_id, "exit 3", None)
        .await
        .expect("execute process");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "failed");
    assert!(terminal.output.is_none());
    assert!(
        terminal
            .error
            .as_deref()
            .unwrap_or("")
            .contains("process failed for task")
    );

    let states = executor.process_states.read().await;
    let state = states.get(&execution_id).expect("process state");
    assert!(matches!(state, ProcessStatus::Failed));
}

#[tokio::test]
async fn test_execute_process_spawn_failure_emits_terminal_failure() {
    let missing_dir = std::env::temp_dir().join(format!("czar-missing-{}", Uuid::new_v4()));
    let executor = ProcessExecutor::new(Some(missing_dir.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(task_id, "echo should-not-run", None)
        .await
        .expect("execute process should still return execution id");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "failed");
    assert!(
        terminal
            .error
            .as_deref()
            .unwrap_or("")
            .contains("failed to spawn process")
    );

    let states = executor.process_states.read().await;
    let state = states.get(&execution_id).expect("process state");
    assert!(matches!(state, ProcessStatus::Failed));
}

#[tokio::test]
async fn test_execute_process_success_without_output_payload() {
    let executor = ProcessExecutor::new(None);
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(task_id, "printf 'just-logs\\n'", None)
        .await
        .expect("execute process");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    assert!(terminal.output.is_none());
    assert!(terminal.error.is_none());
}

#[tokio::test]
async fn test_execute_process_passes_env_values_to_child() {
    let executor = ProcessExecutor::new(None);
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(
            task_id,
            "printf '%s %s\\n' \"$LOG_LEVEL\" \"$MAX_RETRIES\"",
            Some(serde_json::json!({
                "task_name": "demo",
                "env": {
                    "LOG_LEVEL": "debug",
                    "MAX_RETRIES": 3
                }
            })),
        )
        .await
        .expect("execute process");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    assert!(
        updates
            .iter()
            .any(|u| { u.status == "log" && u.log.as_deref().unwrap_or("").contains("debug 3") })
    );
    assert_eq!(updates.last().expect("terminal update").status, "success");
}

#[tokio::test]
async fn test_execute_process_succeeds_without_status_channel() {
    let executor = ProcessExecutor::new(None);
    let task_id = Uuid::new_v4();
    let execution_id = executor
        .execute_process(task_id, "printf 'no-channel\\n'", None)
        .await
        .expect("execute process");

    timeout(Duration::from_secs(2), async {
        loop {
            let states = executor.process_states.read().await;
            if matches!(states.get(&execution_id), Some(ProcessStatus::Success)) {
                break;
            }
            drop(states);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("process should complete");
}

#[tokio::test]
async fn test_execute_process_uses_pre_mounted_secret_files() {
    let base = std::env::temp_dir().join(format!("czar-secret-file-test-{}", Uuid::new_v4()));
    let secret_path = ".czar/secrets/service-account.json";
    fs::create_dir_all(base.join(".czar/secrets")).expect("create secret dir");
    fs::write(base.join(secret_path), "super-secret-json").expect("write mounted secret file");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(
            task_id,
            "cat \"$CZAR_SECRET_SERVICE_ACCOUNT_PATH\"",
            Some(serde_json::json!({
                "task_name": "secret-file-task",
                "secrets": [
                    {
                        "name": "service-account",
                        "secret": "projects/demo/secrets/service-account",
                        "path": secret_path
                    }
                ]
            })),
        )
        .await
        .expect("execute process");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    assert!(
        updates.iter().any(|update| {
            update.status == "log"
                && update
                    .log
                    .as_deref()
                    .unwrap_or("")
                    .contains("super-secret-json")
        }),
        "secret file content should be visible to the child process"
    );
    assert_eq!(updates.last().expect("terminal update").status, "success");

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        base.join(secret_path).exists(),
        "infra-mounted secret file should be left in place"
    );

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn test_execute_process_runs_custom_python_executor() {
    let _guard = python_test_lock().lock().await;
    let base = std::env::temp_dir().join(format!("czar-custom-exec-test-{}", Uuid::new_v4()));
    fs::create_dir_all(base.join("executors")).expect("create executor dir");
    fs::write(
        base.join("pyproject.toml"),
        "[project]\nname = \"custom-exec-test\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
    fs::write(base.join("executors").join("__init__.py"), "").expect("write package init");
    fs::write(
        base.join("executors").join("echo_exec.py"),
        r#"
import json
from pathlib import Path

class EchoExecutor:
    def __init__(self, config):
        self.suffix = config.get("suffix", "!")

    def run(self, config_file, ctx, task_input):
        config = json.loads(Path(config_file).read_text())
        ctx.log.info(f"running custom executor for {task_input['name']}")
        return {"message": f"{config['prefix']} {task_input['name']}{self.suffix}"}
"#,
    )
    .expect("write executor");
    fs::write(
        base.join("query.json"),
        serde_json::json!({"prefix": "hello"}).to_string(),
    )
    .expect("write config file");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(
            task_id,
            "",
            Some(serde_json::json!({
                "task_name": "custom-exec-task",
                "task_input": { "name": "czar" },
                "config_file": base.join("query.json").to_string_lossy().to_string(),
                "python_path": base.to_string_lossy().to_string(),
                "custom_executor": {
                    "type": "executors.echo_exec:EchoExecutor",
                    "config": { "suffix": "?" }
                }
            })),
        )
        .await
        .expect("execute custom executor");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    assert_eq!(
        terminal.output,
        Some(serde_json::json!({"message": "hello czar?"}))
    );

    let _ = fs::remove_dir_all(&base);
}
