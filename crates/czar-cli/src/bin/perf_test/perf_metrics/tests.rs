use super::*;

fn metric(
    ts: u64,
    runs: u128,
    tasks: u128,
    updates: u128,
    sleep: u128,
    total: u128,
) -> SchedulerMetrics {
    SchedulerMetrics {
        timestamp: ts,
        process_pending_runs_ms: runs,
        process_pending_tasks_ms: tasks,
        process_status_updates_ms: updates,
        sleep_ms: sleep,
        total_loop_ms: total,
        runs_processed: 0,
        tasks_processed: 0,
        status_updates_processed: 0,
    }
}

#[test]
fn test_status_counts_from_rows_maps_known_values() {
    let counts = status_counts_from_rows(vec![
        ("pending".to_string(), 3),
        ("running".to_string(), 2),
        ("success".to_string(), 7),
        ("failed".to_string(), 1),
        ("dispatched".to_string(), 4),
        ("unknown".to_string(), 99),
    ]);
    assert_eq!(
        counts,
        StatusCounts {
            pending: 3,
            dispatched: 4,
            running: 2,
            success: 7,
            failed: 1,
        }
    );
}

#[test]
fn test_parse_metrics_line_handles_missing_and_invalid_payloads() {
    assert!(parse_metrics_line("random line").is_none());
    assert!(parse_metrics_line("SCHEDULER_METRICS: {").is_none());

    let parsed = parse_metrics_line(
        r#"INFO SCHEDULER_METRICS: {"timestamp":1,"process_pending_runs_ms":2,"process_pending_tasks_ms":3,"process_status_updates_ms":4,"sleep_ms":5,"total_loop_ms":6}"#,
    )
    .expect("should parse metrics");
    assert_eq!(parsed.timestamp, 1);
    assert_eq!(parsed.total_loop_ms, 6);
}

#[test]
fn test_append_metrics_from_reader_collects_only_valid_lines() {
    let input = "\
noise
SCHEDULER_METRICS: {\"timestamp\":10,\"process_pending_runs_ms\":1,\"process_pending_tasks_ms\":1,\"process_status_updates_ms\":1,\"sleep_ms\":1,\"total_loop_ms\":4}
SCHEDULER_METRICS: {
SCHEDULER_METRICS: {\"timestamp\":11,\"process_pending_runs_ms\":2,\"process_pending_tasks_ms\":3,\"process_status_updates_ms\":4,\"sleep_ms\":1,\"total_loop_ms\":10}
";
    let mut all = VecDeque::new();
    append_metrics_from_reader(std::io::Cursor::new(input), &mut all);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].timestamp, 10);
    assert_eq!(all[1].timestamp, 11);
}

#[test]
fn test_summarize_and_format_metrics() {
    let mut all = VecDeque::new();
    all.push_back(metric(10, 100, 200, 300, 400, 1000));
    all.push_back(metric(20, 50, 100, 150, 200, 500));

    let summary = summarize_metrics(&all).expect("summary");
    assert_eq!(summary.total_runs_ms, 150);
    assert_eq!(summary.total_tasks_ms, 300);
    assert_eq!(summary.total_status_updates_ms, 450);
    assert_eq!(summary.total_sleep_ms, 600);
    assert_eq!(summary.total_loop_ms, 1500);
    assert_eq!(summary.loops, 2);
    assert_eq!(summary.last_timestamp, 20);

    let text = format_metrics_display(&all);
    assert!(text.contains("runs:0.1s/10.0%"));
    assert!(text.contains("tasks:0.3s/20.0%"));
    assert!(text.contains("updates:0.5s/30.0%") || text.contains("updates:0.4s/30.0%"));
    assert!(text.contains("sleep:0.6s/40.0%"));
    assert!(text.contains("total:1.5s (2 loops)"));
}

#[test]
fn test_format_metrics_display_empty() {
    let all = VecDeque::new();
    assert_eq!(format_metrics_display(&all), "waiting for metrics...");
    assert!(summarize_metrics(&all).is_none());
}
