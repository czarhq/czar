use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
};

use crate::app::App;

use super::style::{fuzzy_highlight_spans, status_span, truncate};

pub(super) fn render(f: &mut Frame, area: Rect, app: &App) {
    // Center the popup: up to 80 cols wide, sized to fit results
    let width = (area.width * 70 / 100).clamp(40, 80);
    let results = app.cp_results();
    let list_height = (results.len() as u16).min(15);
    let total_height = 3 + list_height; // border + search bar + border + rows
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(total_height + 4) / 3;
    let popup_area = Rect::new(x, y, width, total_height.max(4));

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Search Runs (↑↓ navigate  Enter select  Esc close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Split inner area: search bar (1 line) + results list
    let inner_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    render_search_bar(f, inner_rows[0], app);
    render_results(f, inner_rows[1], app, &results);
}

fn render_search_bar(f: &mut Frame, area: Rect, app: &App) {
    let line = Line::from(vec![
        Span::styled("  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            app.cp_query.as_str().to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("█", Style::default().fg(Color::Cyan)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_results(f: &mut Frame, area: Rect, app: &App, results: &[&crate::api::Run]) {
    if results.is_empty() {
        f.render_widget(
            Paragraph::new("  no matches").style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let hl = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let rows: Vec<Row> = results
        .iter()
        .map(|run| {
            let id_short = &run.id[..run.id.len().min(8)];
            let ts = crate::app::search::fmt_datetime_short(&run.created_at);

            let id_spans = fuzzy_highlight_spans(
                id_short,
                &app.cp_query,
                Style::default().fg(Color::DarkGray),
            );
            let name_spans = fuzzy_highlight_spans(
                &truncate(&run.workflow, 28),
                &app.cp_query,
                Style::default(),
            );
            let ts_spans =
                fuzzy_highlight_spans(&ts, &app.cp_query, Style::default().fg(Color::DarkGray));

            Row::new(vec![
                Cell::from(Line::from(id_spans)),
                Cell::from(Line::from(name_spans)),
                Cell::from(Line::from(vec![status_span(&run.status)])),
                Cell::from(Line::from(ts_spans)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Fill(1),
            Constraint::Length(9),
            Constraint::Length(11),
        ],
    )
    .row_highlight_style(hl)
    .column_spacing(1);

    let mut state = TableState::default().with_selected(Some(app.cp_selected));
    f.render_stateful_widget(table, area, &mut state);
}
