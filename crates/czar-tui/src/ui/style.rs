use std::collections::HashSet;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::app::search::fuzzy_match_positions;

pub(super) fn status_color(status: &str) -> Color {
    match status {
        s if s.contains("success") || s.contains("succeed") => Color::Green,
        s if s.contains("run") || s.contains("progress") || s.contains("active") => Color::Yellow,
        s if s.contains("fail") || s.contains("error") => Color::Red,
        s if s.contains("cancel") => Color::Magenta,
        s if s.contains("pause") => Color::Blue,
        s if s.contains("wait") || s.contains("queue") || s.contains("pending") => Color::DarkGray,
        s if s.contains("skip") => Color::DarkGray,
        _ => Color::Gray,
    }
}

pub(super) fn status_span(status: &str) -> Span<'static> {
    Span::styled(
        status.to_string(),
        Style::default()
            .fg(status_color(status))
            .add_modifier(Modifier::BOLD),
    )
}

/// Condense an ISO8601 timestamp to "MM-DD HH:MM".
pub(super) fn fmt_datetime_short(s: &str) -> String {
    let month_day = s.get(5..10).unwrap_or("");
    let hhmm = s.get(11..16).unwrap_or("");
    if month_day.is_empty() {
        return s.get(..16).unwrap_or(s).to_string();
    }
    format!("{month_day} {hhmm}")
}

pub(super) fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = chars[..max_chars.saturating_sub(1)].iter().collect();
        format!("{truncated}…")
    }
}

/// Build spans for `text` with matched query characters highlighted in yellow.
pub(super) fn fuzzy_highlight_spans(
    text: &str,
    query: &str,
    base_style: Style,
) -> Vec<Span<'static>> {
    let Some(positions) = fuzzy_match_positions(text, query) else {
        return vec![Span::styled(text.to_string(), base_style)];
    };
    if positions.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    let match_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let chars: Vec<char> = text.chars().collect();
    let matched: HashSet<usize> = positions.into_iter().collect();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut in_match = false;

    for (i, c) in chars.iter().enumerate() {
        let is_match = matched.contains(&i);
        if is_match != in_match && !buf.is_empty() {
            spans.push(Span::styled(
                buf.clone(),
                if in_match { match_style } else { base_style },
            ));
            buf.clear();
        }
        in_match = is_match;
        buf.push(*c);
    }
    if !buf.is_empty() {
        spans.push(Span::styled(
            buf,
            if in_match { match_style } else { base_style },
        ));
    }
    spans
}

pub(super) fn log_line_to_ratatui(raw: &str) -> Line<'static> {
    let (color, rest) = if let Some(r) = raw.strip_prefix("O ") {
        (None, r)
    } else if let Some(r) = raw.strip_prefix("E ") {
        (Some(Color::Red), r)
    } else if let Some(r) = raw.strip_prefix("S ") {
        (Some(Color::Blue), r)
    } else {
        (Some(Color::DarkGray), raw)
    };
    let text = strip_log_ts(rest).to_string();
    match color {
        None => Line::from(text),
        Some(c) => Line::from(Span::styled(text, Style::default().fg(c))),
    }
}

fn strip_log_ts(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() > 13 && b[2] == b':' && b[5] == b':' && b[8] == b'.' && b[12] == b' ' {
        &s[13..]
    } else {
        s
    }
}
