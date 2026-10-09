use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::VecDeque;
use std::io::{BufReader, Seek, SeekFrom};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::time::sleep;

#[path = "perf_test/db_proxy.rs"]
mod db_proxy;
#[path = "perf_test/db_stats.rs"]
mod db_stats;
#[cfg(test)]
#[path = "perf_test/integration_tests.rs"]
mod integration_tests;
#[path = "perf_test/perf_metrics.rs"]
mod perf_metrics;
#[path = "perf_test/perf_support.rs"]
mod perf_support;

#[derive(Parser, Default)]
#[command(name = "perf-test")]
#[command(about = "Performance testing tool for czar")]
struct Args {
    /// Config file to load (from perf-configs/*.yaml)
    #[arg(short, long)]
    config: String,
}

#[derive(Debug, Default, Deserialize, serde::Serialize)]
struct PerfConfig {
    workflows: u32,
    tasks_per_workflow: u32,
    duration: f32,
    scheduler: SchedulerConfig,
    /// Simulated DB round-trip latency in milliseconds (0 = no simulation).
    /// When set, perf-test spawns a local TCP proxy that adds this delay to
    /// every scheduler→DB packet. The scheduler process is pointed at the proxy
    /// instead of Postgres directly. Has no effect on prod deployments.
    #[serde(default)]
    db_extra_latency_ms: u64,
    /// Fixed measurement window in seconds (0 = disabled).
    /// When set, pre-queues `workflows` runs then measures tasks/sec for exactly
    /// this many seconds, regardless of how many tasks complete. Set `workflows`
    /// high enough that the queue never drains before the window ends.
    #[serde(default)]
    benchmark_duration_secs: u64,
    /// Hard timeout in seconds for the monitoring loop (0 = no timeout).
    /// If tasks haven't all completed within this time, the test aborts and
    /// prints partial results. Prevents runaway benchmarks on high-latency configs.
    #[serde(default)]
    timeout_secs: u64,
}

#[derive(Debug, Default, Deserialize, serde::Serialize)]
struct SchedulerConfig {
    poll_interval_secs: f64,
    max_tasks_per_batch: i64,
    max_concurrent_dispatches: usize,
    max_concurrent_status_checks: usize,
    #[serde(default = "default_db_pool_size")]
    db_pool_size: u32,
    #[serde(default = "default_enable_triggerer")]
    enable_triggerer: bool,
}

fn default_db_pool_size() -> u32 {
    10
}

fn default_enable_triggerer() -> bool {
    true
}

#[derive(Debug, Default)]
struct ResourceStats {
    scheduler_rss_kb: u64,
    scheduler_cpu_percent: f32,
}

#[derive(Debug, Default, Deserialize)]
struct SchedulerMetrics {
    timestamp: u64,
    process_pending_runs_ms: u128,
    process_pending_tasks_ms: u128,
    process_status_updates_ms: u128,
    sleep_ms: u128,
    total_loop_ms: u128,
    #[serde(default)]
    runs_processed: usize,
    #[serde(default)]
    tasks_processed: usize,
    #[serde(default)]
    status_updates_processed: usize,
}

async fn run(args: Args) -> Result<()> {
    let config = perf_support::load_config_by_name(&args.config)?;
    let database_url = perf_support::resolve_database_url();
    let czar_bin = perf_support::resolve_czar_binary();

    let pool = PgPool::connect(&database_url).await?;

    println!("=== Performance Test Configuration ===");
    println!("Config: {}", args.config);
    println!("Runs to trigger: {}", config.workflows);
    println!("Tasks per run: {}", config.tasks_per_workflow);
    println!("Task duration: {}s", config.duration);
    println!(
        "Total tasks: {}",
        config.workflows * config.tasks_per_workflow
    );
    if config.db_extra_latency_ms > 0 {
        println!(
            "DB extra latency: {}ms (simulated)",
            config.db_extra_latency_ms
        );
    }
    if config.benchmark_duration_secs > 0 {
        println!(
            "Benchmark window: {}s (throughput measured over fixed duration)",
            config.benchmark_duration_secs
        );
    } else if config.timeout_secs > 0 {
        println!("Hard timeout: {}s", config.timeout_secs);
    }
    println!();

    // Create test script
    println!("Creating test script...");
    let perf_task_script = perf_support::ensure_perf_task_script(config.duration)?;
    let perf_workflow_yaml =
        perf_support::write_perf_workflow_yaml(&perf_task_script, config.tasks_per_workflow)?;

    // Clean old workflow
    println!("Creating performance test workflow...");
    sqlx::query(
        r#"
        DELETE FROM tasks
        WHERE run_id IN (
            SELECT r.id
            FROM runs r
            JOIN workflows w ON r.workflow_id = w.id
            WHERE w.name = 'perf-test'
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        DELETE FROM runs
        WHERE workflow_id IN (
            SELECT id FROM workflows WHERE name = 'perf-test'
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM workflows WHERE name = 'perf-test'")
        .execute(&pool)
        .await?;

    // Load workflow
    let create_status = Command::new(&czar_bin)
        .args(["load", perf_workflow_yaml.to_string_lossy().as_ref()])
        .env("DATABASE_URL", &database_url)
        .status()?;

    if !create_status.success() {
        anyhow::bail!("Failed to load workflow");
    }

    // Write scheduler config to temp file
    let scheduler_config_path = perf_support::write_scheduler_config(&config.scheduler)?;

    // Start scheduler in background with config
    println!("Starting scheduler...");
    println!("  Poll interval: {}s", config.scheduler.poll_interval_secs);
    println!("  Max batch: {}", config.scheduler.max_tasks_per_batch);
    println!(
        "  Max concurrent dispatches: {}",
        config.scheduler.max_concurrent_dispatches
    );
    println!(
        "  Max concurrent status checks: {}",
        config.scheduler.max_concurrent_status_checks
    );

    // Optionally start a DB proxy for latency simulation
    let mut _db_proxy: Option<db_proxy::DbProxy> = None;
    let scheduler_db_url = if config.db_extra_latency_ms > 0 {
        match db_proxy::parse_upstream_addr(&database_url) {
            Ok(upstream) => {
                match db_proxy::DbProxy::start(upstream, config.db_extra_latency_ms).await {
                    Ok(proxy) => {
                        let proxied_url =
                            db_proxy::rewrite_url_to_proxy(&database_url, proxy.local_addr);
                        println!(
                            "  DB latency proxy: {}ms delay → {} via {}",
                            config.db_extra_latency_ms, upstream, proxy.local_addr
                        );
                        _db_proxy = Some(proxy);
                        proxied_url
                    }
                    Err(e) => {
                        println!(
                            "Warning: could not start DB proxy: {e}. Running without latency simulation."
                        );
                        database_url.clone()
                    }
                }
            }
            Err(e) => {
                println!(
                    "Warning: could not parse DB URL for proxy: {e}. Running without latency simulation."
                );
                database_url.clone()
            }
        }
    } else {
        database_url.clone()
    };

    // Redirect scheduler logs to file so we can read them
    let scheduler_log_path = format!("/tmp/czar-scheduler-{}.log", std::process::id());
    let log_file = std::fs::File::create(&scheduler_log_path)?;

    let mut scheduler = Command::new(&czar_bin)
        .args(["serve", "--config", &scheduler_config_path])
        .env("RUST_LOG", "info,czar_core::scheduler=debug")
        .env("DATABASE_URL", &scheduler_db_url)
        .env("CZAR_DB_TRACE", "1")
        .stdout(log_file.try_clone()?)
        .stderr(log_file)
        .spawn()?;

    let scheduler_pid = Pid::from_u32(scheduler.id());
    sleep(perf_support::scheduler_boot_wait()).await;

    // Reset pg_stat_statements for a clean baseline (if available)
    let pg_stats_available = db_stats::is_pg_stat_statements_available(&pool).await;
    if pg_stats_available {
        match db_stats::reset_pg_stat_statements(&pool).await {
            Ok(()) => println!("pg_stat_statements reset for clean baseline"),
            Err(e) => println!("Warning: could not reset pg_stat_statements: {e}"),
        }
    }

    // Get initial memory stats
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::Some(&[scheduler_pid]), true);
    let scheduler_rss_start = system
        .process(scheduler_pid)
        .map(|p| p.memory() / 1024) // Convert bytes to KB
        .unwrap_or(0);

    // Trigger workflows
    println!();
    println!("=== Starting Performance Test ===");
    let start_time = Instant::now();

    let mut trigger_handles = Vec::new();
    for _ in 0..config.workflows {
        let czar_bin = czar_bin.clone();
        let db_url = database_url.clone();
        let handle = tokio::spawn(async move {
            Command::new(&czar_bin)
                .args(["trigger", "perf-test"])
                .env("DATABASE_URL", &db_url)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
        });
        trigger_handles.push(handle);
    }

    for handle in trigger_handles {
        handle.await??;
    }

    let trigger_end = Instant::now();
    let trigger_duration = trigger_end.duration_since(start_time).as_secs_f64();

    println!("All runs triggered");

    // Monitor completion
    println!("Monitoring task completion...");
    let total_tasks = config.workflows * config.tasks_per_workflow;
    let mut completed = 0u32;
    let mut all_metrics: VecDeque<SchedulerMetrics> = VecDeque::new();
    let mut log_position: u64 = 0;
    let max_monitor_polls = perf_support::monitor_max_polls();
    let mut monitor_polls = 0usize;

    // CPU and peak task-count tracking
    let mut cpu_samples: Vec<f32> = Vec::new();
    let mut peak_pending: i64 = 0;
    let mut peak_running: i64 = 0;
    let mut peak_dispatched: i64 = 0;

    let benchmark_mode = config.benchmark_duration_secs > 0;
    loop {
        // In benchmark mode, loop for the full window even if all tasks finish early.
        // In normal mode, exit as soon as all tasks complete.
        if !benchmark_mode && completed >= total_tasks {
            break;
        }
        sleep(Duration::from_millis(500)).await;
        monitor_polls += 1;

        // Get task counts by status
        let status_counts: Vec<(String, i64)> =
            sqlx::query_as("SELECT status, COUNT(*) FROM tasks GROUP BY status")
                .fetch_all(&pool)
                .await?;
        let counts = perf_metrics::status_counts_from_rows(status_counts);
        completed = (counts.success + counts.failed) as u32;
        peak_pending = peak_pending.max(counts.pending);
        peak_running = peak_running.max(counts.running);
        peak_dispatched = peak_dispatched.max(counts.dispatched);

        // Get resource stats
        system.refresh_processes(ProcessesToUpdate::Some(&[scheduler_pid]), true);
        let resource_stats = if let Some(process) = system.process(scheduler_pid) {
            let cpu = process.cpu_usage();
            if cpu > 0.0 {
                cpu_samples.push(cpu);
            }
            ResourceStats {
                scheduler_rss_kb: process.memory() / 1024,
                scheduler_cpu_percent: cpu,
            }
        } else {
            ResourceStats {
                scheduler_rss_kb: 0,
                scheduler_cpu_percent: 0.0,
            }
        };

        let elapsed = start_time.elapsed().as_secs_f64();

        // Read new lines from log file since last position
        if let Ok(mut file) = std::fs::File::open(&scheduler_log_path)
            && file.seek(SeekFrom::Start(log_position)).is_ok()
        {
            let reader = BufReader::new(&file);
            perf_metrics::append_metrics_from_reader(reader, &mut all_metrics);
            // Update position to current end of file
            if let Ok(metadata) = std::fs::metadata(&scheduler_log_path) {
                log_position = metadata.len();
            }
        }

        // Show cumulative metrics with both absolute values and percentages
        let metrics_display = perf_metrics::format_metrics_display(&all_metrics);

        println!(
            "[{:.1}s] Completed:{}/{} | Pending:{} Dispatched:{} Running:{} | Scheduler: {} KB, {:.1}% CPU | {}",
            elapsed,
            completed,
            total_tasks,
            counts.pending,
            counts.dispatched,
            counts.running,
            resource_stats.scheduler_rss_kb,
            resource_stats.scheduler_cpu_percent,
            metrics_display
        );

        if let Some(max_polls) = max_monitor_polls
            && monitor_polls >= max_polls
            && completed < total_tasks
        {
            println!(
                "Stopping monitor after {} polls before completion (CZAR_PERF_MONITOR_MAX_POLLS={})",
                monitor_polls, max_polls
            );
            break;
        }

        if benchmark_mode && start_time.elapsed().as_secs() >= config.benchmark_duration_secs {
            println!(
                "Benchmark window complete: {}s elapsed, {}/{} tasks completed.",
                config.benchmark_duration_secs, completed, total_tasks
            );
            break;
        }

        if !benchmark_mode
            && config.timeout_secs > 0
            && start_time.elapsed().as_secs() >= config.timeout_secs
            && completed < total_tasks
        {
            println!(
                "Hard timeout reached after {}s ({}/{} tasks completed). Printing partial results.",
                config.timeout_secs, completed, total_tasks
            );
            break;
        }
    }

    let end_time = Instant::now();
    let total_duration = end_time.duration_since(start_time).as_secs_f64();

    println!();
    println!();
    println!("=== Performance Test Results ===");

    // Analyze scheduler metrics
    if let Some(summary) = perf_metrics::summarize_metrics(&all_metrics) {
        println!();
        for line in perf_support::scheduler_breakdown_lines(summary) {
            println!("{line}");
        }
    }

    // Query latency stats
    let latency_stats: (i64, Option<f64>, Option<f64>, Option<f64>) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as tasks,
            AVG(EXTRACT(EPOCH FROM (finished_at - created_at)))::FLOAT8 as avg_latency_sec,
            MIN(EXTRACT(EPOCH FROM (finished_at - created_at)))::FLOAT8 as min_latency_sec,
            MAX(EXTRACT(EPOCH FROM (finished_at - created_at)))::FLOAT8 as max_latency_sec
        FROM tasks
        WHERE finished_at IS NOT NULL
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let latency_lines = perf_support::latency_lines(latency_stats);
    if !latency_lines.is_empty() {
        println!();
        for line in latency_lines {
            println!("{line}");
        }
    }

    // Calculate throughput — in benchmark mode use completed tasks (not total pre-queued).
    let measured_tasks = if benchmark_mode {
        completed
    } else {
        total_tasks
    };
    let (run_submission_rate, task_throughput) = perf_support::throughput(
        config.workflows,
        measured_tasks,
        trigger_duration,
        total_duration,
    );

    println!();
    for line in perf_support::throughput_lines(
        run_submission_rate,
        task_throughput,
        total_duration,
        config.workflows,
        config.tasks_per_workflow,
        measured_tasks,
    ) {
        println!("{line}");
    }

    // Final resource stats
    system.refresh_processes(ProcessesToUpdate::Some(&[scheduler_pid]), true);
    let scheduler_rss = system
        .process(scheduler_pid)
        .map(|p| p.memory() / 1024)
        .unwrap_or(0);

    let avg_cpu = if cpu_samples.is_empty() {
        0.0f32
    } else {
        cpu_samples.iter().sum::<f32>() / cpu_samples.len() as f32
    };
    let peak_cpu = cpu_samples.iter().cloned().fold(0.0f32, f32::max);

    println!();
    for line in perf_support::resource_lines(
        scheduler_rss,
        scheduler_rss_start,
        avg_cpu,
        peak_cpu,
        peak_pending,
        peak_running,
        peak_dispatched,
    ) {
        println!("{line}");
    }

    // Query pg_stat_statements for DB call analysis
    if pg_stats_available {
        println!();
        println!("DB Query Stats (pg_stat_statements):");
        match db_stats::query_top_statements(&pool, 15).await {
            Ok(stats) => {
                for line in db_stats::format_db_stats(&stats) {
                    println!("{line}");
                }
            }
            Err(e) => println!("  Warning: could not query pg_stat_statements: {e}"),
        }
    }

    // DB call profile (from CZAR_DB_TRACE instrumentation)
    println!();
    println!("DB Call Profile (CZAR_DB_TRACE):");
    let db_profile = db_stats::parse_db_trace_log(&scheduler_log_path);
    for line in db_stats::format_db_profile(&db_profile) {
        println!("{line}");
    }

    // Cleanup
    println!();
    println!("Cleaning up...");
    let _ = scheduler.kill();
    let _ = std::fs::remove_file(&perf_workflow_yaml);
    let _ = std::fs::remove_file(&scheduler_config_path);
    let _ = std::fs::remove_file(&scheduler_log_path);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    run(Args::parse()).await
}

#[cfg(test)]
#[path = "perf-test/tests.rs"]
mod tests;
