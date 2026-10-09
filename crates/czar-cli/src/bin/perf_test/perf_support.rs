use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{PerfConfig, SchedulerConfig};
use crate::perf_metrics::MetricsSummary;

pub fn load_config_from_path(path: &Path) -> Result<PerfConfig> {
    let config_content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file {}: {}", path.display(), e))?;
    serde_yaml::from_str::<PerfConfig>(&config_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse config file {}: {}", path.display(), e))
}

pub fn load_config_by_name(name: &str) -> Result<PerfConfig> {
    let config_path = PathBuf::from(format!("perf-configs/{}.yaml", name));
    load_config_from_path(&config_path)
}

pub fn resolve_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/orchestrator".to_string())
}

pub fn resolve_czar_binary() -> String {
    std::env::var("CZAR_PERF_BIN").unwrap_or_else(|_| "../../target/release/czar".to_string())
}

pub fn scheduler_boot_wait() -> Duration {
    match std::env::var("CZAR_PERF_BOOT_WAIT_SECS") {
        Ok(raw) => match raw.parse::<f64>() {
            Ok(secs) if secs.is_finite() && secs >= 0.0 => Duration::from_secs_f64(secs),
            _ => Duration::from_secs(2),
        },
        Err(_) => Duration::from_secs(2),
    }
}

pub fn monitor_max_polls() -> Option<usize> {
    let raw = std::env::var("CZAR_PERF_MONITOR_MAX_POLLS").ok()?;
    let parsed = raw.parse::<usize>().ok()?;
    (parsed > 0).then_some(parsed)
}

pub fn ensure_perf_task_script(duration: f32) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("czar-perf-task-{}.sh", std::process::id()));
    std::fs::write(&path, format!("#!/bin/bash\nsleep {}\n", duration))?;
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&path)
        .status()?;
    Ok(path)
}

pub fn write_perf_workflow_yaml(task_script: &Path, tasks_per_workflow: u32) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("czar-perf-workflow-{}.yaml", std::process::id()));
    let tasks = (0..tasks_per_workflow)
        .map(|index| {
            format!(
                r#"  task_{index}:
    executor: process
    command: {command}
"#,
                command = task_script.to_string_lossy()
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let yaml = format!(
        r#"name: perf-test
description: Performance test workflow
tasks:
{tasks}"#
    );
    std::fs::write(&path, yaml)?;
    Ok(path)
}

pub fn write_scheduler_config(scheduler: &SchedulerConfig) -> Result<String> {
    let scheduler_config_path = format!("/tmp/czar-scheduler-{}.yaml", std::process::id());
    let scheduler_config_yaml = serde_yaml::to_string(scheduler)?;
    std::fs::write(&scheduler_config_path, scheduler_config_yaml)?;
    Ok(scheduler_config_path)
}

pub fn throughput(
    workflows: u32,
    total_tasks: u32,
    trigger_duration: f64,
    total_duration: f64,
) -> (f64, f64) {
    let task_throughput = total_tasks as f64 / total_duration;
    let run_submission_rate = workflows as f64 / trigger_duration;
    (run_submission_rate, task_throughput)
}

pub fn scheduler_breakdown_lines(summary: MetricsSummary) -> Vec<String> {
    vec![
        "Scheduler Time Breakdown:".to_string(),
        format!("  Total scheduler loops: {}", summary.loops),
        format!(
            "  Time processing runs: {:.2}s ({:.1}%, {} runs)",
            summary.total_runs_ms as f64 / 1000.0,
            summary.runs_pct(),
            summary.total_runs_processed,
        ),
        format!(
            "  Time processing tasks: {:.2}s ({:.1}%, {} tasks dispatched)",
            summary.total_tasks_ms as f64 / 1000.0,
            summary.tasks_pct(),
            summary.total_tasks_processed,
        ),
        format!(
            "  Time processing status updates: {:.2}s ({:.1}%, {} updates)",
            summary.total_status_updates_ms as f64 / 1000.0,
            summary.status_updates_pct(),
            summary.total_status_updates_processed,
        ),
        format!(
            "  Time sleeping: {:.2}s ({:.1}%)",
            summary.total_sleep_ms as f64 / 1000.0,
            summary.sleep_pct()
        ),
        format!(
            "  Total scheduler time: {:.2}s",
            summary.total_loop_ms as f64 / 1000.0
        ),
    ]
}

pub fn resource_lines(
    rss_kb: u64,
    rss_start_kb: u64,
    avg_cpu_pct: f32,
    peak_cpu_pct: f32,
    peak_pending: i64,
    peak_running: i64,
    peak_dispatched: i64,
) -> Vec<String> {
    vec![
        "Resource Usage:".to_string(),
        format!("  Scheduler RSS: {} KB", rss_kb),
        format!(
            "  Scheduler RSS growth: {} KB",
            rss_kb.saturating_sub(rss_start_kb)
        ),
        format!("  Scheduler CPU avg: {:.1}%", avg_cpu_pct),
        format!("  Scheduler CPU peak: {:.1}%", peak_cpu_pct),
        format!(
            "  Peak task queue: {} pending, {} dispatched, {} running",
            peak_pending, peak_dispatched, peak_running
        ),
    ]
}

pub fn latency_lines(latency_stats: (i64, Option<f64>, Option<f64>, Option<f64>)) -> Vec<String> {
    if let (tasks, Some(avg), Some(min), Some(max)) = latency_stats {
        vec![
            "Latency Stats:".to_string(),
            format!("  Tasks: {}", tasks),
            format!("  Avg latency: {:.3}s", avg),
            format!("  Min latency: {:.3}s", min),
            format!("  Max latency: {:.3}s", max),
        ]
    } else {
        Vec::new()
    }
}

pub fn throughput_lines(
    run_submission_rate: f64,
    task_throughput: f64,
    total_duration: f64,
    workflows: u32,
    tasks_per_workflow: u32,
    total_tasks: u32,
) -> Vec<String> {
    vec![
        "Throughput:".to_string(),
        format!("  Run submission: {:.2} runs/sec", run_submission_rate),
        format!("  Task completion: {:.2} tasks/sec", task_throughput),
        format!("  Total duration: {:.2}s", total_duration),
        "Explanation:".to_string(),
        format!(
            "  - {} runs created ({} tasks each = {} total tasks)",
            workflows, tasks_per_workflow, total_tasks
        ),
        "  - Run submission measures how fast we enqueue work".to_string(),
        "  - Task completion measures actual work throughput".to_string(),
    ]
}

#[cfg(test)]
#[path = "perf_support/tests.rs"]
mod tests;
