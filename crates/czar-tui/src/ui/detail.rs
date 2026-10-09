use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table, Wrap,
        canvas::{Canvas, Line as CLine},
    },
};

use crate::app::App;
use crate::dag;

use super::detail_support::{progress_bar_line, workflow_summary_lines};
use super::style::{fmt_datetime_short, log_line_to_ratatui, status_color, status_span};

pub(crate) fn parse_progress_counts(p: &serde_json::Value) -> Option<(u64, u64, Option<String>)> {
    super::detail_support::parse_progress_counts(p)
}

pub(super) fn render_run_task_list(f: &mut Frame, area: Rect, app: &App) {
    let Some(ref detail) = app.run_detail else {
        f.render_widget(
            Paragraph::new("  Loading…").style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    // Task list
    let hl = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let rows: Vec<Row> = detail
        .tasks
        .iter()
        .map(|t| {
            let started = t
                .started_at
                .as_deref()
                .map(fmt_datetime_short)
                .unwrap_or_default();
            let status_cell = if t.status == "running" {
                if let Some(ref p) = t.progress {
                    if let Some((current, total, _)) = parse_progress_counts(p) {
                        let pct = current * 100 / total;
                        Cell::from(Span::styled(
                            format!("{pct:>3}%"),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ))
                    } else {
                        Cell::from(Line::from(status_span(&t.status)))
                    }
                } else {
                    Cell::from(Line::from(status_span(&t.status)))
                }
            } else {
                Cell::from(Line::from(status_span(&t.status)))
            };
            Row::new(vec![
                Cell::from(t.name.clone()),
                status_cell,
                Cell::from(started).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Length(9),
            Constraint::Length(11),
        ],
    )
    .row_highlight_style(hl)
    .column_spacing(1);
    f.render_stateful_widget(table, cols[0], &mut app.task_list_state.clone());

    // Log view
    render_task_log(f, cols[1], app, &detail.tasks);
}

fn render_task_log(f: &mut Frame, area: Rect, app: &App, tasks: &[crate::api::Task]) {
    let block = Block::default()
        .title(" Logs ")
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let selected_task = app.task_list_state.selected().and_then(|i| tasks.get(i));

    let Some(task) = selected_task else {
        // Virtual scroll: count lines cheaply, then build only the visible window.
        let task_line_count = |t: &crate::api::Task| -> usize {
            2 // header + blank separator
            + t.progress.as_ref()
                .and_then(parse_progress_counts)
                .map(|_| 1)
                .unwrap_or(0)
            + t.error.as_ref().map(|s| s.lines().count()).unwrap_or(0)
            + t.logs.as_ref().map(|s| s.lines().count()).unwrap_or(0)
            + if t.error.is_none() && t.logs.is_none() { 1 } else { 0 }
        };
        let total: usize = tasks.iter().map(task_line_count).sum();
        let scroll = (app.task_log_scroll as usize).min(total.saturating_sub(1));
        let capacity = (inner.height as usize).saturating_add(4);
        let mut lines: Vec<Line> = Vec::with_capacity(capacity);
        let mut skip = scroll;

        'outer: for t in tasks {
            // Header
            if skip > 0 {
                skip -= 1;
            } else {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("── {} ", t.name),
                        Style::default()
                            .fg(status_color(&t.status))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("[{}]", t.status),
                        Style::default().fg(status_color(&t.status)),
                    ),
                ]));
                if lines.len() >= capacity {
                    break 'outer;
                }
            }
            // Progress bar (running tasks with progress)
            if let Some(ref p) = t.progress
                && let Some((current, total, message)) = parse_progress_counts(p)
            {
                if skip > 0 {
                    skip -= 1;
                } else {
                    lines.push(progress_bar_line(current, total, message.as_deref()));
                    if lines.len() >= capacity {
                        break 'outer;
                    }
                }
            }
            if let Some(ref err) = t.error {
                for l in err.lines() {
                    if skip > 0 {
                        skip -= 1;
                        continue;
                    }
                    lines.push(Line::from(Span::styled(
                        l.to_string(),
                        Style::default().fg(Color::Red),
                    )));
                    if lines.len() >= capacity {
                        break 'outer;
                    }
                }
            }
            if let Some(ref logs) = t.logs {
                for l in logs.lines() {
                    if skip > 0 {
                        skip -= 1;
                        continue;
                    }
                    lines.push(log_line_to_ratatui(l));
                    if lines.len() >= capacity {
                        break 'outer;
                    }
                }
            }
            if t.error.is_none() && t.logs.is_none() {
                if skip > 0 {
                    skip -= 1;
                } else {
                    lines.push(Line::from(Span::styled(
                        "  (no output)",
                        Style::default().fg(Color::DarkGray),
                    )));
                    if lines.len() >= capacity {
                        break 'outer;
                    }
                }
            }
            // Blank separator
            if skip > 0 {
                skip -= 1;
            } else {
                lines.push(Line::from(""));
                if lines.len() >= capacity {
                    break 'outer;
                }
            }
        }

        f.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            inner,
        );
        if total > inner.height as usize {
            let mut sb = ScrollbarState::new(total).position(scroll);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                inner,
                &mut sb,
            );
        }
        return;
    };

    let mut lines: Vec<Line> = Vec::new();

    if let Some(ref p) = task.progress
        && let Some((current, total, message)) = parse_progress_counts(p)
    {
        lines.push(progress_bar_line(current, total, message.as_deref()));
        lines.push(Line::from(""));
    }

    if let Some(ref err) = task.error {
        lines.push(Line::from(Span::styled(
            "Error:",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        for l in err.lines() {
            lines.push(Line::from(Span::styled(
                l.to_string(),
                Style::default().fg(Color::Red),
            )));
        }
        lines.push(Line::from(""));
    }
    if let Some(ref logs) = task.logs {
        lines.push(Line::from(Span::styled(
            "Logs:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        for l in logs.lines() {
            lines.push(log_line_to_ratatui(l));
        }
        lines.push(Line::from(""));
    }
    if let Some(ref output) = task.output {
        lines.push(Line::from(Span::styled(
            "Output:",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));
        let pretty = serde_json::to_string_pretty(output).unwrap_or_else(|_| output.to_string());
        for l in pretty.lines() {
            lines.push(Line::from(l.to_string()));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no output)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let total = lines.len() as u16;
    let scroll = app.task_log_scroll.min(total.saturating_sub(1));
    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        inner,
    );
    if total > inner.height {
        let mut sb = ScrollbarState::new(total as usize).position(scroll as usize);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            inner,
            &mut sb,
        );
    }
}

pub(super) fn render_workflow_task_list(f: &mut Frame, area: Rect, app: &App) {
    let Some(ref detail) = app.workflow_detail else {
        f.render_widget(
            Paragraph::new("  Loading…").style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    };

    let summary_lines = workflow_summary_lines(detail, &app.workflow_last_run);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(summary_lines.len() as u16),
            Constraint::Min(0),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Text::from(summary_lines)).wrap(Wrap { trim: false }),
        sections[0],
    );

    let hl = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let rows: Vec<Row> = detail
        .tasks
        .iter()
        .map(|t| {
            let deps = if t.depends_on.is_empty() {
                String::from("—")
            } else {
                t.depends_on.join(", ")
            };
            Row::new(vec![
                Cell::from(t.name.clone()),
                Cell::from(deps).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from("Task").style(
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Depends On").style(
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let table = Table::new(rows, [Constraint::Percentage(40), Constraint::Fill(1)])
        .header(header)
        .row_highlight_style(hl)
        .column_spacing(1);

    f.render_stateful_widget(table, sections[1], &mut app.task_list_state.clone());
}

pub(super) fn render_dag(
    f: &mut Frame,
    area: Rect,
    app: &App,
    layout: &dag::DagLayout,
    statuses: &[String],
) {
    if layout.nodes.is_empty() {
        f.render_widget(
            Paragraph::new("  No tasks.").style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    // Track rendered area for pan clamping (Cell interior mutability).
    app.last_dag_area.set(area);

    let selected = app.task_list_state.selected();
    let pan_x = app.dag_pan_x;
    let pan_y = app.dag_pan_y;
    let aw = area.width as f64;
    let ah = area.height as f64;
    let top_margin = 1.0f64;

    let nw = dag::NW;
    let nh = dag::NH;

    // Canvas coordinate transforms with panning.
    let to_cx = |sx: f64| sx - pan_x;
    let to_cy = |sy: f64| ah - top_margin - (sy - pan_y);

    let inner_w = nw as usize - 2;
    let border_h = "─".repeat(inner_w);

    #[derive(Default)]
    struct DrawnNode {
        cx: f64,
        cy_top: f64,
        cy_mid: f64,
        cy_bot: f64,
        top: String,
        mid: String,
        bot: String,
        color: Color,
        selected: bool,
    }
    #[derive(Default)]
    struct DrawnEdge {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    }

    let drawn_nodes: Vec<DrawnNode> = layout
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            node.x + nw > pan_x
                && node.x < pan_x + aw
                && node.y + nh > pan_y
                && node.y < pan_y + ah + top_margin
        })
        .map(|(i, node)| {
            let is_sel = selected == Some(i);
            let empty = String::new();
            let st = statuses.get(i).unwrap_or(&empty);
            let color = if is_sel {
                Color::Black
            } else if st.is_empty() {
                Color::Cyan
            } else {
                status_color(st)
            };
            DrawnNode {
                cx: to_cx(node.x),
                cy_top: to_cy(node.y),
                cy_mid: to_cy(node.y + 1.0),
                cy_bot: to_cy(node.y + 2.0),
                top: format!("┌{}┐", border_h),
                mid: format!("│ {} │", node.label),
                bot: format!("└{}┘", border_h),
                color,
                selected: is_sel,
            }
        })
        .collect();

    let drawn_edges: Vec<DrawnEdge> = layout
        .edges
        .iter()
        .filter(|e| {
            let ex_min = e.x1.min(e.x2);
            let ex_max = e.x1.max(e.x2);
            let ey_min = e.y1.min(e.y2);
            let ey_max = e.y1.max(e.y2);
            ex_max > pan_x
                && ex_min < pan_x + aw
                && ey_max > pan_y
                && ey_min < pan_y + ah + top_margin
        })
        .map(|e| DrawnEdge {
            x1: to_cx(e.x1),
            y1: to_cy(e.y1),
            x2: to_cx(e.x2),
            y2: to_cy(e.y2),
        })
        .collect();

    let canvas = Canvas::default()
        .x_bounds([0.0, aw])
        .y_bounds([0.0, ah])
        .paint(move |ctx| {
            for e in &drawn_edges {
                ctx.draw(&CLine {
                    x1: e.x1,
                    y1: e.y1,
                    x2: e.x2,
                    y2: e.y2,
                    color: Color::DarkGray,
                });
            }
            for node in &drawn_nodes {
                let bg = if node.selected {
                    Color::Cyan
                } else {
                    Color::Reset
                };
                let style = Style::default().fg(node.color).bg(bg);
                ctx.print(
                    node.cx,
                    node.cy_top,
                    Line::from(Span::styled(node.top.clone(), style)),
                );
                ctx.print(
                    node.cx,
                    node.cy_mid,
                    Line::from(Span::styled(node.mid.clone(), style)),
                );
                ctx.print(
                    node.cx,
                    node.cy_bot,
                    Line::from(Span::styled(node.bot.clone(), style)),
                );
            }
        });

    f.render_widget(canvas, area);
}
