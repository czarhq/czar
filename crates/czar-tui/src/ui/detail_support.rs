use std::collections::HashMap;

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::api::WorkflowInfo;

use super::style::{fmt_datetime_short, status_span};

/// Extract `(current, total, message)` from a progress JSON value.
pub(crate) fn parse_progress_counts(p: &serde_json::Value) -> Option<(u64, u64, Option<String>)> {
    parse_progress(p)
}

fn parse_progress(p: &serde_json::Value) -> Option<(u64, u64, Option<String>)> {
    let current = p.get("current")?.as_u64()?;
    let total = p.get("total")?.as_u64()?;
    if total == 0 {
        return None;
    }
    let message = p
        .get("message")
        .and_then(|m| m.as_str())
        .map(str::to_string);
    Some((current, total, message))
}

pub(super) fn progress_bar_line(current: u64, total: u64, message: Option<&str>) -> Line<'static> {
    const BAR_WIDTH: u64 = 20;
    let filled = ((current * BAR_WIDTH) / total).min(BAR_WIDTH) as usize;
    let bar: String = "█".repeat(filled) + &"░".repeat(BAR_WIDTH as usize - filled);
    let pct = current * 100 / total;
    let text = match message {
        Some(m) if !m.is_empty() => format!("[{bar}] {current}/{total} ({pct}%) — {m}"),
        _ => format!("[{bar}] {current}/{total} ({pct}%)"),
    };
    Line::from(Span::styled(text, Style::default().fg(Color::Yellow)))
}

pub(super) fn workflow_summary_lines(
    detail: &WorkflowInfo,
    workflow_last_run: &HashMap<String, (String, Option<String>)>,
) -> Vec<Line<'static>> {
    let mut summary_lines: Vec<Line> = Vec::new();
    if let Some((status, dt)) = workflow_last_run.get(&detail.name) {
        let mut spans = vec![
            Span::styled("Last run: ", Style::default().fg(Color::DarkGray)),
            status_span(status),
        ];
        if let Some(dt) = dt.as_deref() {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                fmt_datetime_short(dt),
                Style::default().fg(Color::White),
            ));
        }
        summary_lines.push(Line::from(spans));
    } else {
        summary_lines.push(Line::from(vec![
            Span::styled("Last run: ", Style::default().fg(Color::DarkGray)),
            Span::styled("never", Style::default().fg(Color::DarkGray)),
        ]));
    }
    if let Some(schedule) = detail.schedule.as_deref() {
        let mut spans = vec![
            Span::styled("Schedule: ", Style::default().fg(Color::DarkGray)),
            Span::styled(schedule.to_string(), Style::default().fg(Color::Yellow)),
        ];
        if detail.schedule_enabled {
            if let Some(next) = detail.next_scheduled_at.as_deref() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled("Next: ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(
                    fmt_datetime_short(next),
                    Style::default().fg(Color::White),
                ));
            }
        } else {
            spans.push(Span::raw("  "));
            spans.push(Span::styled("paused", Style::default().fg(Color::DarkGray)));
        }
        summary_lines.push(Line::from(spans));
    }
    summary_lines
}
