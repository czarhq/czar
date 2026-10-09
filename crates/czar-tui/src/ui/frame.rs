use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, Focus, RunProgress, SidebarTab};

use super::command_palette;
use super::detail;
use super::sidebar;

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(0),    // content
            Constraint::Length(1), // keybinding hints
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_header(f, rows[0], app);
    render_content(f, rows[1], app);
    render_hints(f, rows[2], app);
    render_status(f, rows[3], app);

    if app.command_palette {
        command_palette::render(f, area, app);
    }

    crate::debug_dump::maybe_write_render_dump(f.buffer_mut());
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let focused_label = match app.focus {
        Focus::Sidebar => match app.sidebar_tab {
            SidebarTab::Workflows => "Workflows",
            SidebarTab::Runs => "Runs",
        },
        Focus::Detail => "Detail",
    };
    let text = Line::from(vec![
        Span::styled(
            " Czar Control TUI ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  focus: {focused_label}"),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    f.render_widget(
        Paragraph::new(text).style(Style::default().bg(Color::Black)),
        area,
    );
}

fn render_hints(f: &mut Frame, area: Rect, app: &App) {
    let hints: &str = if app.command_palette {
        " type to search runs  ↑↓:navigate  Enter:open  Esc/^P:close"
    } else if app.graph_mode {
        " ↑↓←→:pan  Esc/g:exit graph mode  q:quit"
    } else if app.is_searching {
        " type to filter  ↑↓:navigate  Enter:confirm  Esc:clear"
    } else {
        match app.focus {
            Focus::Sidebar => {
                " ^P:search runs  /:filter  Tab:switch panel  ↑↓:navigate  g:graph  t:trigger  r:reload  q:quit"
            }
            Focus::Detail if app.selected_run.is_some() => {
                " ^P:search  ←/→:focus  ↑↓:task  Space:select/deselect  PgUp/Dn:scroll log  g:graph  c:cancel  p:pause/resume  r:reload  q:quit"
            }
            Focus::Detail => " ^P:search  Esc:sidebar  g:graph  t:trigger  r:reload  q:quit",
        }
    };
    f.render_widget(
        Paragraph::new(hints).style(Style::default().fg(Color::DarkGray).bg(Color::Black)),
        area,
    );
}

fn render_status(f: &mut Frame, area: Rect, app: &App) {
    let msg = format!(" {}", app.status_msg);
    f.render_widget(
        Paragraph::new(msg).style(Style::default().fg(Color::White).bg(Color::DarkGray)),
        area,
    );
}

fn render_content(f: &mut Frame, area: Rect, app: &App) {
    if app.graph_mode {
        render_graph_fullscreen(f, area, app);
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
        .split(area);

    sidebar::render(f, cols[0], app);
    render_detail(f, cols[1], app);
}

fn render_graph_fullscreen(f: &mut Frame, area: Rect, app: &App) {
    let title = match (&app.selected_workflow, &app.selected_run) {
        (_, Some(run_id)) => format!(
            " Graph: run {} — Esc/g to exit ",
            &run_id[..run_id.len().min(8)]
        ),
        (Some(wf), _) => format!(" Graph: {wf} — Esc/g to exit "),
        _ => String::from(" Graph — Esc/g to exit "),
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(ref layout) = app.dag_layout {
        let statuses: Vec<String> = app
            .run_detail
            .as_ref()
            .map(|d| d.tasks.iter().map(|t| t.status.clone()).collect())
            .unwrap_or_else(|| vec![String::new(); layout.nodes.len()]);
        detail::render_dag(f, inner, app, layout, &statuses);
    } else {
        f.render_widget(
            Paragraph::new("  Computing layout…").style(Style::default().fg(Color::DarkGray)),
            inner,
        );
    }
}

fn render_detail(f: &mut Frame, area: Rect, app: &App) {
    let focused = matches!(app.focus, Focus::Detail);
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let title = match (&app.selected_workflow, &app.selected_run) {
        (_, Some(run_id)) => {
            let short = &run_id[..run_id.len().min(8)];
            if let Some(ref detail) = app.run_detail {
                let status = &detail.run.status;
                if status == "running" && !detail.tasks.is_empty() {
                    if let Some(p) = app.run_progress_cache.get(run_id) {
                        let pct = (p.fraction * 100.0).round() as u64;
                        let fail_suffix = if p.failed > 0 {
                            format!("  ✗{}", p.failed)
                        } else {
                            String::new()
                        };
                        format!(
                            " Run: {short}  [{status}  {}/{}{fail_suffix}  {pct}%] ",
                            p.done, p.total
                        )
                    } else {
                        format!(" Run: {short}  [{status}] ")
                    }
                } else {
                    format!(" Run: {short}  [{status}] ")
                }
            } else {
                format!(" Run: {short}  […] ")
            }
        }
        (Some(wf), None) => format!(" Workflow: {wf} "),
        _ => String::from(" Tasks "),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.selected_run.is_some() {
        detail::render_run_task_list(f, inner, app);
    } else if app.selected_workflow.is_some() {
        detail::render_workflow_task_list(f, inner, app);
    } else {
        f.render_widget(
            Paragraph::new("  Select a workflow or run from the sidebar.")
                .style(Style::default().fg(Color::DarkGray)),
            inner,
        );
    }
}

// Shared helper used by sidebar for running-run progress display.
pub(super) fn run_progress_cell(p: &RunProgress) -> ratatui::text::Line<'static> {
    let pct = (p.fraction * 100.0).round() as u64;
    let base = format!("{}/{} {}%", p.done, p.total, pct);
    let base_span = Span::styled(
        base,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    if p.failed > 0 {
        ratatui::text::Line::from(vec![
            base_span,
            Span::styled(
                format!(" ✗{}", p.failed),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        ratatui::text::Line::from(base_span)
    }
}
