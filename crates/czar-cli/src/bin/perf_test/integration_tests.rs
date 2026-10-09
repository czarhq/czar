use super::*;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

async fn async_env_lock() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
}

fn restore_env_var(name: &str, previous: Option<String>) {
    match previous {
        Some(value) => unsafe { std::env::set_var(name, value) },
        None => unsafe { std::env::remove_var(name) },
    }
}

async fn resolve_test_database_url() -> Option<String> {
    static RESOLVED_URL: OnceLock<Option<String>> = OnceLock::new();
    if let Some(cached) = RESOLVED_URL.get() {
        return cached.clone();
    }

    // Keep integration tests responsive when local Postgres is down/misconfigured.
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
    let mut candidates = Vec::new();
    if let Ok(url) = std::env::var("CZAR_POSTGRES_TEST_URL") {
        candidates.push(url);
    }
    if let Ok(url) = std::env::var("DATABASE_URL") {
        candidates.push(url);
    }
    candidates.push("postgres://postgres:postgres@localhost:5432/orchestrator".to_string());
    candidates.push(perf_support::resolve_database_url());

    for candidate in candidates {
        let connect_result =
            tokio::time::timeout(CONNECT_TIMEOUT, PgPool::connect(&candidate)).await;
        if matches!(connect_result, Ok(Ok(_))) {
            let resolved = Some(candidate);
            let _ = RESOLVED_URL.set(resolved.clone());
            return resolved;
        }
    }

    let _ = RESOLVED_URL.set(None);
    None
}

async fn ensure_perf_tables(database_url: &str) -> bool {
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
    let pool = match tokio::time::timeout(CONNECT_TIMEOUT, PgPool::connect(database_url)).await {
        Ok(Ok(pool)) => pool,
        Ok(Err(err)) => {
            eprintln!("skipping perf-test integration path test: could not connect to db: {err}");
            return false;
        }
        Err(_) => {
            eprintln!("skipping perf-test integration path test: timed out connecting to db");
            return false;
        }
    };

    if let Err(err) =
        sqlx::query("CREATE TABLE IF NOT EXISTS workflows (id TEXT PRIMARY KEY, name TEXT UNIQUE)")
            .execute(&pool)
            .await
    {
        eprintln!(
            "skipping perf-test integration path test: failed to ensure workflows table: {err}"
        );
        return false;
    }

    if let Err(err) =
        sqlx::query("CREATE TABLE IF NOT EXISTS runs (id TEXT PRIMARY KEY, workflow_id TEXT)")
            .execute(&pool)
            .await
    {
        eprintln!("skipping perf-test integration path test: failed to ensure runs table: {err}");
        return false;
    }

    if let Err(err) = sqlx::query(
        "CREATE TABLE IF NOT EXISTS tasks (id BIGSERIAL PRIMARY KEY, run_id TEXT, status TEXT, created_at TIMESTAMPTZ, finished_at TIMESTAMPTZ)",
    )
    .execute(&pool)
    .await
    {
        eprintln!("skipping perf-test integration path test: failed to ensure tasks table: {err}");
        return false;
    }

    true
}

fn write_perf_config(name: &str, workflows: u32, tasks_per_workflow: u32) -> String {
    let path = format!("perf-configs/{}.yaml", name);
    let config_yaml = format!(
        r#"
workflows: {workflows}
tasks_per_workflow: {tasks_per_workflow}
duration: 0.01
scheduler:
  poll_interval_secs: 0.01
  max_tasks_per_batch: 10
  max_concurrent_dispatches: 2
  max_concurrent_status_checks: 2
"#
    );
    std::fs::write(&path, config_yaml).expect("write test config");
    path
}

fn write_mock_czar_binary(
    emit_metrics: bool,
    fail_load_workflow: bool,
    run_exits: bool,
) -> PathBuf {
    let fake_bin_path = std::env::temp_dir().join(format!("czar-mock-{}", Uuid::new_v4()));
    let load_workflow_body = if fail_load_workflow {
        "exit 1"
    } else {
        "exit 0"
    };
    let run_body = if emit_metrics {
        r#"echo 'SCHEDULER_METRICS: {"timestamp":1,"process_pending_runs_ms":1,"process_pending_tasks_ms":2,"process_status_updates_ms":3,"sleep_ms":4,"total_loop_ms":10}'"#
    } else {
        ":"
    };
    let run_tail = if run_exits {
        "exit 0"
    } else {
        "while true; do sleep 1; done"
    };
    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
cmd="${{1:-}}"
case "$cmd" in
  load) {load_workflow_body} ;;
  run)
    {run_body}
    {run_tail}
    ;;
  trigger) exit 0 ;;
  *) exit 0 ;;
esac
"#
    );
    std::fs::write(&fake_bin_path, script).expect("write mock czar binary");
    Command::new("chmod")
        .args(["+x", fake_bin_path.to_string_lossy().as_ref()])
        .status()
        .expect("chmod mock binary");
    fake_bin_path
}

async fn run_with_mock(
    workflows: u32,
    tasks_per_workflow: u32,
    monitor_max_polls: Option<usize>,
    emit_metrics: bool,
    fail_load_workflow: bool,
    run_exits: bool,
) -> Option<anyhow::Result<()>> {
    let Some(database_url) = resolve_test_database_url().await else {
        eprintln!(
            "skipping perf-test integration path test: could not resolve a reachable postgres url"
        );
        return None;
    };
    if !ensure_perf_tables(&database_url).await {
        return None;
    }

    let config_name = format!("test-{}", Uuid::new_v4());
    let config_path = write_perf_config(&config_name, workflows, tasks_per_workflow);
    let fake_bin_path = write_mock_czar_binary(emit_metrics, fail_load_workflow, run_exits);

    let previous_bin = std::env::var("CZAR_PERF_BIN").ok();
    let previous_boot_wait = std::env::var("CZAR_PERF_BOOT_WAIT_SECS").ok();
    let previous_db_url = std::env::var("DATABASE_URL").ok();
    let previous_monitor_max = std::env::var("CZAR_PERF_MONITOR_MAX_POLLS").ok();
    unsafe {
        std::env::set_var("CZAR_PERF_BIN", fake_bin_path.to_string_lossy().to_string());
        std::env::set_var("CZAR_PERF_BOOT_WAIT_SECS", "0");
        std::env::set_var("DATABASE_URL", database_url);
        match monitor_max_polls {
            Some(v) => std::env::set_var("CZAR_PERF_MONITOR_MAX_POLLS", v.to_string()),
            None => std::env::remove_var("CZAR_PERF_MONITOR_MAX_POLLS"),
        }
    }

    let result = run(Args {
        config: config_name.clone(),
    })
    .await;

    restore_env_var("CZAR_PERF_BIN", previous_bin);
    restore_env_var("CZAR_PERF_BOOT_WAIT_SECS", previous_boot_wait);
    restore_env_var("DATABASE_URL", previous_db_url);
    restore_env_var("CZAR_PERF_MONITOR_MAX_POLLS", previous_monitor_max);
    let _ = std::fs::remove_file(config_path);
    let _ = std::fs::remove_file(fake_bin_path);
    Some(result)
}

#[tokio::test]
async fn test_run_with_mock_czar_binary_and_zero_workflows() {
    let _guard = async_env_lock().await;
    let Some(result) = run_with_mock(0, 1, None, false, false, false).await else {
        return;
    };
    assert!(
        result.is_ok(),
        "expected perf-test run to succeed: {result:?}"
    );
}

#[tokio::test]
async fn test_run_with_monitor_cap_exercises_polling_paths() {
    let _guard = async_env_lock().await;
    let Some(result) = run_with_mock(5, 5, Some(1), true, false, true).await else {
        return;
    };
    assert!(
        result.is_ok(),
        "expected perf-test run to succeed: {result:?}"
    );
}

#[tokio::test]
async fn test_run_errors_when_load_workflow_fails() {
    let _guard = async_env_lock().await;
    let Some(result) = run_with_mock(1, 1, Some(1), false, true, true).await else {
        return;
    };
    assert!(result.is_err(), "expected perf-test run to fail");
}
