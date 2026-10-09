use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
};

use crate::app::{App, Focus, SidebarTab};

use super::frame::run_progress_cell;
use super::style::{fmt_datetime_short, fuzzy_highlight_spans, status_span, truncate};

pub(super) fn render(f: &mut Frame, area: Rect, app: &App) {
    let focused = matches!(app.focus, Focus::Sidebar);
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tabs
            Constraint::Length(1), // search bar
            Constraint::Min(0),    // list
        ])
        .split(inner);

    render_tabs(f, rows[0], app, focused);
    render_search_bar(f, rows[1], app);

    match app.sidebar_tab {
        SidebarTab::Workflows => render_workflow_list(f, rows[2], app),
        SidebarTab::Runs => render_run_list(f, rows[2], app),
    }
}

fn render_tabs(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let tab_titles = vec![
        format!("Workflows ({})", app.workflows_total),
        format!("Runs ({})", app.runs_total),
    ];
    let selected = match app.sidebar_tab {
        SidebarTab::Workflows => 0,
        SidebarTab::Runs => 1,
    };
    let active_style = if focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Gray)
            .add_modifier(Modifier::BOLD)
    };
    let tabs = Tabs::new(tab_titles)
        .select(selected)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(active_style)
        .divider(" │ ");
    f.render_widget(tabs, area);
}

fn render_search_bar(f: &mut Frame, area: Rect, app: &App) {
    let (prefix_style, content, suffix) = if app.is_searching {
        (
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            app.search_query.as_str(),
            "█",
        )
    } else if !app.search_query.is_empty() {
        (
            Style::default().fg(Color::Yellow),
            app.search_query.as_str(),
            "",
        )
    } else {
        (Style::default().fg(Color::DarkGray), "type / to search", "")
    };

    let line = Line::from(vec![
        Span::styled("/ ", Style::default().fg(Color::DarkGray)),
        Span::styled(content, prefix_style),
        Span::styled(suffix, Style::default().fg(Color::Cyan)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_workflow_list(f: &mut Frame, area: Rect, app: &App) {
    let hl = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let rows: Vec<Row> = app
        .filtered_workflows()
        .into_iter()
        .map(|wf| {
            let mut name_spans =
                fuzzy_highlight_spans(&wf.name, &app.search_query, Style::default());
            if let (Some(_), true) = (&wf.schedule, wf.schedule_enabled) {
                name_spans.push(Span::styled(" ⏰", Style::default().fg(Color::Yellow)));
            } else if wf.schedule.is_some() {
                name_spans.push(Span::styled(" ⏸", Style::default().fg(Color::DarkGray)));
            }

            let (status_cell, dt_cell) =
                if let Some((status, dt)) = app.workflow_last_run.get(&wf.name) {
                    let sc = Cell::from(Line::from(status_span(status)));
                    let dc = Cell::from(dt.as_deref().map(fmt_datetime_short).unwrap_or_default())
                        .style(Style::default().fg(Color::DarkGray));
                    (sc, dc)
                } else {
                    (
                        Cell::from("—").style(Style::default().fg(Color::DarkGray)),
                        Cell::from(""),
                    )
                };

            Row::new(vec![
                Cell::from(Line::from(name_spans)),
                status_cell,
                dt_cell,
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

    f.render_stateful_widget(table, area, &mut app.workflow_list_state.clone());
}

fn render_run_list(f: &mut Frame, area: Rect, app: &App) {
    let hl = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let rows: Vec<Row> = app
        .filtered_runs()
        .into_iter()
        .map(|run| {
            let name_spans = fuzzy_highlight_spans(
                &truncate(&run.workflow, 16),
                &app.search_query,
                Style::default(),
            );
            let status_cell = if run.status == "running" {
                if let Some(p) = app.run_progress_cache.get(&run.id) {
                    Cell::from(run_progress_cell(p))
                } else {
                    Cell::from(Line::from(status_span(&run.status)))
                }
            } else {
                Cell::from(Line::from(status_span(&run.status)))
            };
            let dt = run.started_at.as_deref().unwrap_or(&run.created_at);
            let dt_cell =
                Cell::from(fmt_datetime_short(dt)).style(Style::default().fg(Color::DarkGray));
            Row::new(vec![
                Cell::from(Line::from(name_spans)),
                status_cell,
                dt_cell,
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(11),
        ],
    )
    .row_highlight_style(hl)
    .column_spacing(1);

    f.render_stateful_widget(table, area, &mut app.run_list_state.clone());
}
