use std::collections::VecDeque;
use std::io::BufRead;

use super::SchedulerMetrics;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StatusCounts {
    pub pending: i64,
    pub dispatched: i64,
    pub running: i64,
    pub success: i64,
    pub failed: i64,
}

pub fn status_counts_from_rows(rows: Vec<(String, i64)>) -> StatusCounts {
    let mut counts = StatusCounts::default();
    for (status, count) in rows {
        match status.as_str() {
            "pending" => counts.pending = count,
            "dispatched" => counts.dispatched = count,
            "running" => counts.running = count,
            "success" => counts.success = count,
            "failed" => counts.failed = count,
            _ => {}
        }
    }
    counts
}

pub fn parse_metrics_line(line: &str) -> Option<SchedulerMetrics> {
    let json_start = line.find("SCHEDULER_METRICS: ")?;
    let json_str = &line[json_start + "SCHEDULER_METRICS: ".len()..];
    serde_json::from_str::<SchedulerMetrics>(json_str).ok()
}

pub fn append_metrics_from_reader<R: BufRead>(
    reader: R,
    all_metrics: &mut VecDeque<SchedulerMetrics>,
) {
    for line in reader.lines().map_while(Result::ok) {
        if let Some(metrics) = parse_metrics_line(&line) {
            all_metrics.push_back(metrics);
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MetricsSummary {
    pub total_runs_ms: u128,
    pub total_tasks_ms: u128,
    pub total_status_updates_ms: u128,
    pub total_sleep_ms: u128,
    pub total_loop_ms: u128,
    pub loops: usize,
    pub last_timestamp: u64,
    pub total_runs_processed: usize,
    pub total_tasks_processed: usize,
    pub total_status_updates_processed: usize,
}

impl MetricsSummary {
    fn pct(part: u128, total: u128) -> f64 {
        if total == 0 {
            0.0
        } else {
            (part as f64 / total as f64) * 100.0
        }
    }

    pub fn runs_pct(self) -> f64 {
        Self::pct(self.total_runs_ms, self.total_loop_ms)
    }

    pub fn tasks_pct(self) -> f64 {
        Self::pct(self.total_tasks_ms, self.total_loop_ms)
    }

    pub fn status_updates_pct(self) -> f64 {
        Self::pct(self.total_status_updates_ms, self.total_loop_ms)
    }

    pub fn sleep_pct(self) -> f64 {
        Self::pct(self.total_sleep_ms, self.total_loop_ms)
    }
}

pub fn summarize_metrics(all_metrics: &VecDeque<SchedulerMetrics>) -> Option<MetricsSummary> {
    if all_metrics.is_empty() {
        return None;
    }

    Some(MetricsSummary {
        total_runs_ms: all_metrics.iter().map(|m| m.process_pending_runs_ms).sum(),
        total_tasks_ms: all_metrics.iter().map(|m| m.process_pending_tasks_ms).sum(),
        total_status_updates_ms: all_metrics
            .iter()
            .map(|m| m.process_status_updates_ms)
            .sum(),
        total_sleep_ms: all_metrics.iter().map(|m| m.sleep_ms).sum(),
        total_loop_ms: all_metrics.iter().map(|m| m.total_loop_ms).sum(),
        loops: all_metrics.len(),
        last_timestamp: all_metrics.back().map(|m| m.timestamp).unwrap_or(0),
        total_runs_processed: all_metrics.iter().map(|m| m.runs_processed).sum(),
        total_tasks_processed: all_metrics.iter().map(|m| m.tasks_processed).sum(),
        total_status_updates_processed: all_metrics
            .iter()
            .map(|m| m.status_updates_processed)
            .sum(),
    })
}

pub fn format_metrics_display(all_metrics: &VecDeque<SchedulerMetrics>) -> String {
    if let Some(s) = summarize_metrics(all_metrics) {
        format!(
            "runs:{:.1}s/{:.1}%({}) tasks:{:.1}s/{:.1}%({}) updates:{:.1}s/{:.1}%({}) sleep:{:.1}s/{:.1}% total:{:.1}s ({} loops)",
            s.total_runs_ms as f64 / 1000.0,
            s.runs_pct(),
            s.total_runs_processed,
            s.total_tasks_ms as f64 / 1000.0,
            s.tasks_pct(),
            s.total_tasks_processed,
            s.total_status_updates_ms as f64 / 1000.0,
            s.status_updates_pct(),
            s.total_status_updates_processed,
            s.total_sleep_ms as f64 / 1000.0,
            s.sleep_pct(),
            s.total_loop_ms as f64 / 1000.0,
            s.loops,
        )
    } else {
        "waiting for metrics...".to_string()
    }
}

#[cfg(test)]
#[path = "perf_metrics/tests.rs"]
mod tests;
