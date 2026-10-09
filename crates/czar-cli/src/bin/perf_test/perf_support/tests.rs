use super::*;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn sample_config() -> PerfConfig {
    PerfConfig {
        workflows: 2,
        tasks_per_workflow: 3,
        duration: 0.5,
        db_extra_latency_ms: 0,
        timeout_secs: 0,
        benchmark_duration_secs: 0,
        scheduler: SchedulerConfig {
            poll_interval_secs: 0.1,
            max_tasks_per_batch: 100,
            max_concurrent_dispatches: 4,
            max_concurrent_status_checks: 8,
            db_pool_size: 10,
            enable_triggerer: true,
        },
    }
}

#[test]
fn test_load_config_from_path_success_and_error() {
    let dir = std::env::temp_dir().join(format!("czar-perf-support-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("cfg.yaml");

    let cfg = sample_config();
    std::fs::write(&path, serde_yaml::to_string(&cfg).expect("serialize cfg")).expect("write cfg");
    let loaded = load_config_from_path(&path).expect("load config");
    assert_eq!(loaded.workflows, 2);
    assert_eq!(loaded.scheduler.max_tasks_per_batch, 100);

    let missing = load_config_from_path(&dir.join("missing.yaml"));
    assert!(missing.is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_load_config_by_name_reads_from_perf_configs_dir() {
    let name = format!("test-{}", Uuid::new_v4());
    let path = std::path::PathBuf::from(format!("perf-configs/{}.yaml", name));
    let cfg = sample_config();
    std::fs::write(&path, serde_yaml::to_string(&cfg).expect("serialize cfg")).expect("write cfg");

    let loaded = load_config_by_name(&name).expect("load by name");
    assert_eq!(loaded.workflows, 2);
    assert_eq!(loaded.tasks_per_workflow, 3);

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_resolve_database_url_prefers_env() {
    let _guard = env_lock().lock().expect("env lock");
    unsafe {
        std::env::set_var("DATABASE_URL", "postgres://original");
    }
    let previous = std::env::var("DATABASE_URL").ok();

    unsafe {
        std::env::set_var("DATABASE_URL", "postgres://env-db");
    }
    assert_eq!(resolve_database_url(), "postgres://env-db");

    unsafe {
        std::env::remove_var("DATABASE_URL");
    }
    assert!(resolve_database_url().contains("localhost:5432/orchestrator"));

    match previous {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
    assert_eq!(resolve_database_url(), "postgres://original");
}

#[test]
fn test_resolve_czar_binary_prefers_env_and_falls_back() {
    let _guard = env_lock().lock().expect("env lock");
    let previous = std::env::var("CZAR_PERF_BIN").ok();
    unsafe {
        std::env::set_var("CZAR_PERF_BIN", "/tmp/mock-czar");
    }
    assert_eq!(resolve_czar_binary(), "/tmp/mock-czar");

    unsafe {
        std::env::remove_var("CZAR_PERF_BIN");
    }
    assert!(resolve_czar_binary().contains("/czar"));

    match previous {
        Some(v) => unsafe { std::env::set_var("CZAR_PERF_BIN", v) },
        None => unsafe { std::env::remove_var("CZAR_PERF_BIN") },
    }
}

#[test]
fn test_scheduler_boot_wait_reads_env_and_handles_invalid_values() {
    let _guard = env_lock().lock().expect("env lock");
    let previous = std::env::var("CZAR_PERF_BOOT_WAIT_SECS").ok();

    unsafe {
        std::env::set_var("CZAR_PERF_BOOT_WAIT_SECS", "0.0");
    }
    assert_eq!(scheduler_boot_wait(), Duration::from_secs(0));

    unsafe {
        std::env::set_var("CZAR_PERF_BOOT_WAIT_SECS", "-1");
    }
    assert_eq!(scheduler_boot_wait(), Duration::from_secs(2));

    unsafe {
        std::env::set_var("CZAR_PERF_BOOT_WAIT_SECS", "not-a-number");
    }
    assert_eq!(scheduler_boot_wait(), Duration::from_secs(2));

    unsafe {
        std::env::remove_var("CZAR_PERF_BOOT_WAIT_SECS");
    }
    assert_eq!(scheduler_boot_wait(), Duration::from_secs(2));

    match previous {
        Some(v) => unsafe { std::env::set_var("CZAR_PERF_BOOT_WAIT_SECS", v) },
        None => unsafe { std::env::remove_var("CZAR_PERF_BOOT_WAIT_SECS") },
    }
}

#[test]
fn test_monitor_max_polls_env_parsing() {
    let _guard = env_lock().lock().expect("env lock");
    let previous = std::env::var("CZAR_PERF_MONITOR_MAX_POLLS").ok();

    unsafe {
        std::env::set_var("CZAR_PERF_MONITOR_MAX_POLLS", "3");
    }
    assert_eq!(monitor_max_polls(), Some(3));

    unsafe {
        std::env::set_var("CZAR_PERF_MONITOR_MAX_POLLS", "0");
    }
    assert_eq!(monitor_max_polls(), None);

    unsafe {
        std::env::set_var("CZAR_PERF_MONITOR_MAX_POLLS", "bad");
    }
    assert_eq!(monitor_max_polls(), None);

    unsafe {
        std::env::remove_var("CZAR_PERF_MONITOR_MAX_POLLS");
    }
    assert_eq!(monitor_max_polls(), None);

    match previous {
        Some(v) => unsafe { std::env::set_var("CZAR_PERF_MONITOR_MAX_POLLS", v) },
        None => unsafe { std::env::remove_var("CZAR_PERF_MONITOR_MAX_POLLS") },
    }
}

#[test]
fn test_ensure_perf_task_script_writes_shell_script() {
    let path = ensure_perf_task_script(1.25).expect("create script");
    let content = std::fs::read_to_string(&path).expect("read script");
    assert!(content.contains("sleep 1.25"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_write_perf_workflow_yaml_writes_process_tasks() {
    let task_script = PathBuf::from("/tmp/perf-task.sh");
    let path = write_perf_workflow_yaml(&task_script, 3).expect("write workflow");
    let content = std::fs::read_to_string(&path).expect("read workflow");
    assert!(content.contains("name: perf-test"));
    assert!(content.contains("task_0:"));
    assert!(content.contains("task_2:"));
    assert!(content.contains("command: /tmp/perf-task.sh"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_write_scheduler_config_and_throughput() {
    let cfg = sample_config().scheduler;
    let path = write_scheduler_config(&cfg).expect("write scheduler cfg");
    let content = std::fs::read_to_string(&path).expect("read scheduler cfg");
    assert!(content.contains("poll_interval_secs"));
    assert!(content.contains("max_tasks_per_batch"));

    let (runs, tasks) = throughput(4, 40, 2.0, 8.0);
    assert_eq!(runs, 2.0);
    assert_eq!(tasks, 5.0);

    let summary = MetricsSummary {
        total_runs_ms: 100,
        total_tasks_ms: 200,
        total_status_updates_ms: 300,
        total_sleep_ms: 400,
        total_loop_ms: 1000,
        loops: 4,
        last_timestamp: 123,
        total_runs_processed: 2,
        total_tasks_processed: 8,
        total_status_updates_processed: 16,
    };
    let scheduler_lines = scheduler_breakdown_lines(summary);
    assert!(
        scheduler_lines
            .iter()
            .any(|line| line.contains("Total scheduler loops: 4"))
    );

    let latency = latency_lines((4, Some(1.2), Some(0.8), Some(2.1)));
    assert!(
        latency
            .iter()
            .any(|line| line.contains("Avg latency: 1.200s"))
    );
    assert!(latency_lines((4, None, Some(1.0), Some(2.0))).is_empty());

    let throughput = throughput_lines(2.0, 5.0, 8.0, 4, 10, 40);
    assert!(
        throughput
            .iter()
            .any(|line| line.contains("Run submission: 2.00"))
    );
    assert!(
        throughput
            .iter()
            .any(|line| line.contains("4 runs created (10 tasks each = 40 total tasks)"))
    );

    let _ = std::fs::remove_file(path);
}
