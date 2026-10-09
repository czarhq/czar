use super::*;
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn test_default_config_has_sane_values() {
    let config = OrchestratorConfig::default();

    assert!(config.poll_interval_secs.is_finite());
    assert!(config.poll_interval_secs > 0.0);
    assert!(config.max_tasks_per_batch > 0);
    assert!(config.max_concurrent_dispatches > 0);
    assert!(config.max_concurrent_status_checks > 0);
    assert!(config.db_pool_size > 0);
    assert!(config.notifications.is_empty());
}

#[test]
fn test_low_resource_profile_values() {
    let config = OrchestratorConfig::low_resource();

    assert_eq!(config.poll_interval_secs, 2.0);
    assert_eq!(config.max_tasks_per_batch, 50);
    assert_eq!(config.max_concurrent_dispatches, 5);
    assert_eq!(config.max_concurrent_status_checks, 20);
    assert_eq!(config.db_pool_size, 5);
    assert!(config.enable_triggerer);
    assert!(config.notifications.is_empty());
}

#[test]
fn test_high_throughput_profile_values() {
    let config = OrchestratorConfig::high_throughput();

    assert_eq!(config.poll_interval_secs, 5.0);
    assert_eq!(config.max_tasks_per_batch, 500);
    assert_eq!(config.max_concurrent_dispatches, 50);
    assert_eq!(config.max_concurrent_status_checks, 100);
    assert_eq!(config.db_pool_size, 20);
    assert!(config.enable_triggerer);
    assert!(config.notifications.is_empty());
}

#[test]
fn test_default_config_reads_env_overrides_and_fallbacks() {
    let _guard = env_lock().lock().expect("env lock");
    let previous = [
        (
            "POLL_INTERVAL_SECS",
            std::env::var("POLL_INTERVAL_SECS").ok(),
        ),
        (
            "MAX_TASKS_PER_BATCH",
            std::env::var("MAX_TASKS_PER_BATCH").ok(),
        ),
        (
            "MAX_CONCURRENT_DISPATCHES",
            std::env::var("MAX_CONCURRENT_DISPATCHES").ok(),
        ),
        (
            "MAX_CONCURRENT_STATUS_CHECKS",
            std::env::var("MAX_CONCURRENT_STATUS_CHECKS").ok(),
        ),
        ("DB_POOL_SIZE", std::env::var("DB_POOL_SIZE").ok()),
        ("ENABLE_TRIGGERER", std::env::var("ENABLE_TRIGGERER").ok()),
    ];

    unsafe {
        std::env::set_var("POLL_INTERVAL_SECS", "0.25");
        std::env::set_var("MAX_TASKS_PER_BATCH", "200");
        std::env::set_var("MAX_CONCURRENT_DISPATCHES", "20");
        std::env::set_var("MAX_CONCURRENT_STATUS_CHECKS", "70");
        std::env::set_var("DB_POOL_SIZE", "15");
        std::env::set_var("ENABLE_TRIGGERER", "false");
    }
    let config = OrchestratorConfig::default();
    assert_eq!(config.poll_interval_secs, 0.25);
    assert_eq!(config.max_tasks_per_batch, 200);
    assert_eq!(config.max_concurrent_dispatches, 20);
    assert_eq!(config.max_concurrent_status_checks, 70);
    assert_eq!(config.db_pool_size, 15);
    assert!(!config.enable_triggerer);

    unsafe {
        std::env::set_var("POLL_INTERVAL_SECS", "not-a-number");
        std::env::set_var("MAX_TASKS_PER_BATCH", "bad");
    }
    let fallback = OrchestratorConfig::default();
    assert_eq!(fallback.poll_interval_secs, 0.5);
    assert_eq!(fallback.max_tasks_per_batch, 100);

    for (key, value) in previous {
        match value {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
    }
}
