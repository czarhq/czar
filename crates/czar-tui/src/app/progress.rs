use crate::api::Task;

use super::state::RunProgress;

pub fn compute_run_progress(tasks: &[Task]) -> RunProgress {
    let total = tasks.len();
    if total == 0 {
        return RunProgress {
            total: 0,
            done: 0,
            failed: 0,
            fraction: 0.0,
        };
    }
    let mut done = 0usize;
    let mut failed = 0usize;
    let mut fraction_sum = 0.0f64;
    for t in tasks {
        let s = t.status.as_str();
        let is_terminal = s.contains("success")
            || s.contains("fail")
            || s.contains("cancel")
            || s.contains("skip");
        if is_terminal {
            done += 1;
            fraction_sum += 1.0;
            if s.contains("fail") {
                failed += 1;
            }
        } else if s == "running"
            && let Some(ref p) = t.progress
        {
            let current = p.get("current").and_then(|v| v.as_u64()).unwrap_or(0);
            let ptotal = p.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
            if ptotal > 0 {
                fraction_sum += (current as f64) / (ptotal as f64);
            }
        }
    }
    RunProgress {
        total,
        done,
        failed,
        fraction: fraction_sum / total as f64,
    }
}

pub(crate) fn current_time_hms() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}
