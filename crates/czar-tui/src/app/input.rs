use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;
use tokio::sync::mpsc;

use super::worker::AppCommand;
use super::{App, Focus, SidebarTab};

impl App {
    // ── Keyboard handling ─────────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: KeyEvent, cmd_tx: &mpsc::Sender<AppCommand>) {
        // Ctrl+P opens the command palette from any mode
        if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.command_palette = !self.command_palette;
            if self.command_palette {
                self.cp_query.clear();
                self.cp_selected = 0;
            }
            return;
        }

        // Handle command palette input
        if self.command_palette {
            match key.code {
                KeyCode::Esc => {
                    self.command_palette = false;
                    self.cp_query.clear();
                    self.cp_selected = 0;
                }
                KeyCode::Backspace => {
                    self.cp_query.pop();
                    self.cp_selected = 0;
                }
                KeyCode::Char(c) => {
                    self.cp_query.push(c);
                    self.cp_selected = 0;
                }
                #[allow(clippy::collapsible_match)]
                KeyCode::Up => {
                    if self.cp_selected > 0 {
                        self.cp_selected -= 1;
                    }
                }
                KeyCode::Down => {
                    let max = self.cp_results().len().saturating_sub(1);
                    if self.cp_selected < max {
                        self.cp_selected += 1;
                    }
                }
                KeyCode::Enter => {
                    let results = self.cp_results();
                    if let Some(run) = results.get(self.cp_selected) {
                        let run_id = run.id.clone();
                        self.command_palette = false;
                        self.cp_query.clear();
                        self.cp_selected = 0;
                        self.sidebar_tab = SidebarTab::Runs;
                        self.focus = Focus::Sidebar;
                        // Find the index in the full run list and select it
                        if let Some(idx) = self.runs.iter().position(|r| r.id == run_id) {
                            self.run_list_state.select(Some(idx));
                        }
                        self.selected_run = Some(run_id.clone());
                        self.run_detail = None;
                        self.task_list_state = ratatui::widgets::TableState::default();
                        self.task_log_scroll = 0;
                        self.last_task_cursor = 0;
                        self.dag_layout = None;
                        self.dag_pan_x = 0.0;
                        self.dag_pan_y = 0.0;
                        self.graph_mode = false;
                        let _ = cmd_tx.try_send(AppCommand::LoadRunDetail(run_id));
                    }
                }
                _ => {}
            }
            return;
        }

        if self.graph_mode {
            match key.code {
                KeyCode::Up => self.pan_dag(0.0, -4.0),
                KeyCode::Down => self.pan_dag(0.0, 4.0),
                KeyCode::Left => self.pan_dag(-10.0, 0.0),
                KeyCode::Right => self.pan_dag(10.0, 0.0),
                KeyCode::Esc | KeyCode::Char('g') => self.graph_mode = false,
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.should_quit = true;
                }
                _ => {}
            }
            return;
        }

        if self.is_searching {
            match key.code {
                KeyCode::Esc => {
                    self.is_searching = false;
                    self.search_query.clear();
                    self.on_search_changed();
                }
                KeyCode::Enter => {
                    self.is_searching = false;
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.on_search_changed();
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.on_search_changed();
                }
                KeyCode::Up => self.nav_up(cmd_tx),
                KeyCode::Down => self.nav_down(cmd_tx),
                KeyCode::PageUp => self.page_up(cmd_tx),
                KeyCode::PageDown => self.page_down(cmd_tx),
                KeyCode::Home => self.jump_home(cmd_tx),
                KeyCode::End => self.jump_end(cmd_tx),
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }

            KeyCode::Char('/') if matches!(self.focus, Focus::Sidebar) => {
                self.is_searching = true;
                self.search_query.clear();
            }

            KeyCode::Tab => self.cycle_sidebar_tab(cmd_tx),
            KeyCode::Left => self.focus = Focus::Sidebar,
            KeyCode::Right => {
                if matches!(self.focus, Focus::Sidebar) {
                    self.focus = Focus::Detail;
                    if self.task_list_state.selected().is_none()
                        && self
                            .run_detail
                            .as_ref()
                            .is_some_and(|d| !d.tasks.is_empty())
                    {
                        self.task_list_state.select(Some(0));
                        self.task_log_scroll = 0;
                    }
                }
            }

            KeyCode::Up => self.nav_up(cmd_tx),
            KeyCode::Down => self.nav_down(cmd_tx),

            KeyCode::Char('g') => self.graph_mode = true,
            KeyCode::Char(' ') if matches!(self.focus, Focus::Detail) => {
                if let Some(i) = self.task_list_state.selected() {
                    self.last_task_cursor = i;
                    self.task_list_state.select(None);
                } else if let Some(ref d) = self.run_detail
                    && !d.tasks.is_empty()
                {
                    let clamped = self.last_task_cursor.min(d.tasks.len() - 1);
                    self.task_list_state.select(Some(clamped));
                }
                self.task_log_scroll = 0;
            }

            KeyCode::Esc if !self.search_query.is_empty() => {
                self.search_query.clear();
                self.on_search_changed();
            }

            KeyCode::Char('r')
                if cmd_tx
                    .try_send(AppCommand::ReloadWorkflowDefinitions)
                    .is_err() =>
            {
                self.status_msg = String::from("Error: command channel full");
            }
            KeyCode::Char('t') => {
                if let Some(ref wf) = self.selected_workflow.clone() {
                    if cmd_tx.try_send(AppCommand::TriggerRun(wf.clone())).is_err() {
                        self.status_msg = String::from("Error: command channel full");
                    } else {
                        self.status_msg = String::from("Triggering…");
                    }
                }
            }
            KeyCode::Char('c') => {
                if let Some(ref run_id) = self.selected_run.clone() {
                    if cmd_tx
                        .try_send(AppCommand::CancelRun(run_id.clone()))
                        .is_err()
                    {
                        self.status_msg = String::from("Error: command channel full");
                    } else {
                        self.status_msg = String::from("Cancelling…");
                    }
                }
            }
            KeyCode::Char('p') => {
                if let Some(ref run_id) = self.selected_run.clone() {
                    let is_paused = self
                        .run_detail
                        .as_ref()
                        .map(|d| d.run.status == "paused")
                        .unwrap_or(false);
                    let cmd = if is_paused {
                        AppCommand::ResumeRun(run_id.clone())
                    } else {
                        AppCommand::PauseRun(run_id.clone())
                    };
                    if cmd_tx.try_send(cmd).is_err() {
                        self.status_msg = String::from("Error: command channel full");
                    }
                }
            }

            KeyCode::PageUp => self.page_up(cmd_tx),
            KeyCode::PageDown => self.page_down(cmd_tx),
            KeyCode::Home => self.jump_home(cmd_tx),
            KeyCode::End => self.jump_end(cmd_tx),

            _ => {}
        }
    }

    fn on_search_changed(&mut self) {
        match self.sidebar_tab {
            SidebarTab::Workflows => {
                let count = self.filtered_workflows().len();
                if count == 0 {
                    self.workflow_list_state.select(None);
                } else {
                    self.workflow_list_state.select(Some(0));
                }
            }
            SidebarTab::Runs => {
                let count = self.filtered_runs().len();
                if count == 0 {
                    self.run_list_state.select(None);
                } else {
                    self.run_list_state.select(Some(0));
                }
            }
        }
    }

    fn cycle_sidebar_tab(&mut self, cmd_tx: &mpsc::Sender<AppCommand>) {
        self.sidebar_tab = match self.sidebar_tab {
            SidebarTab::Workflows => SidebarTab::Runs,
            SidebarTab::Runs => SidebarTab::Workflows,
        };
        self.focus = Focus::Sidebar;
        self.last_task_cursor = 0;
        self.auto_preview(cmd_tx);
    }

    fn page_up(&mut self, cmd_tx: &mpsc::Sender<AppCommand>) {
        match self.focus {
            Focus::Sidebar => match self.sidebar_tab {
                SidebarTab::Workflows => {
                    let i = self.workflow_list_state.selected().unwrap_or(0);
                    self.workflow_list_state.select(Some(i.saturating_sub(10)));
                    self.auto_preview(cmd_tx);
                }
                SidebarTab::Runs => {
                    let i = self.run_list_state.selected().unwrap_or(0);
                    self.run_list_state.select(Some(i.saturating_sub(10)));
                    self.auto_preview(cmd_tx);
                }
            },
            Focus::Detail => {
                if let Some(i) = self.task_list_state.selected() {
                    self.task_list_state.select(Some(i.saturating_sub(10)));
                    self.task_log_scroll = 0;
                } else if self.run_detail.is_some() {
                    self.task_log_scroll = self.task_log_scroll.saturating_sub(5);
                }
            }
        }
    }

    fn page_down(&mut self, cmd_tx: &mpsc::Sender<AppCommand>) {
        match self.focus {
            Focus::Sidebar => match self.sidebar_tab {
                SidebarTab::Workflows => {
                    let len = self.filtered_workflows().len();
                    if len == 0 {
                        return;
                    }
                    let i = self.workflow_list_state.selected().unwrap_or(0);
                    self.workflow_list_state.select(Some((i + 10).min(len - 1)));
                    self.auto_preview(cmd_tx);
                }
                SidebarTab::Runs => {
                    let len = self.filtered_runs().len();
                    if len == 0 {
                        return;
                    }
                    let i = self.run_list_state.selected().unwrap_or(0);
                    self.run_list_state.select(Some((i + 10).min(len - 1)));
                    self.auto_preview(cmd_tx);
                }
            },
            Focus::Detail => {
                if let Some(i) = self.task_list_state.selected() {
                    let len = self.current_task_count();
                    if len == 0 {
                        return;
                    }
                    self.task_list_state.select(Some((i + 10).min(len - 1)));
                    self.task_log_scroll = 0;
                } else if self.run_detail.is_some() {
                    let total = self.all_logs_line_count();
                    let new_scroll =
                        (self.task_log_scroll as usize + 5).min(total.saturating_sub(1));
                    self.task_log_scroll = new_scroll as u16;
                }
            }
        }
    }

    fn jump_home(&mut self, cmd_tx: &mpsc::Sender<AppCommand>) {
        match self.focus {
            Focus::Sidebar => match self.sidebar_tab {
                SidebarTab::Workflows => {
                    if self.filtered_workflows().is_empty() {
                        return;
                    }
                    self.workflow_list_state.select(Some(0));
                    self.auto_preview(cmd_tx);
                }
                SidebarTab::Runs => {
                    if self.filtered_runs().is_empty() {
                        return;
                    }
                    self.run_list_state.select(Some(0));
                    self.auto_preview(cmd_tx);
                }
            },
            Focus::Detail => {
                if self.current_task_count() == 0 {
                    return;
                }
                self.task_list_state.select(Some(0));
                self.task_log_scroll = 0;
            }
        }
    }

    fn jump_end(&mut self, cmd_tx: &mpsc::Sender<AppCommand>) {
        match self.focus {
            Focus::Sidebar => match self.sidebar_tab {
                SidebarTab::Workflows => {
                    let len = self.filtered_workflows().len();
                    if len == 0 {
                        return;
                    }
                    self.workflow_list_state.select(Some(len - 1));
                    self.auto_preview(cmd_tx);
                }
                SidebarTab::Runs => {
                    let len = self.filtered_runs().len();
                    if len == 0 {
                        return;
                    }
                    self.run_list_state.select(Some(len - 1));
                    self.auto_preview(cmd_tx);
                }
            },
            Focus::Detail => {
                let len = self.current_task_count();
                if len == 0 {
                    return;
                }
                self.task_list_state.select(Some(len - 1));
                if self.run_detail.is_some() {
                    self.task_log_scroll = 0;
                }
            }
        }
    }

    fn nav_up(&mut self, cmd_tx: &mpsc::Sender<AppCommand>) {
        match self.focus {
            Focus::Sidebar => match self.sidebar_tab {
                SidebarTab::Workflows => {
                    let i = self.workflow_list_state.selected().unwrap_or(0);
                    if i > 0 {
                        self.workflow_list_state.select(Some(i - 1));
                        self.auto_preview(cmd_tx);
                    }
                }
                SidebarTab::Runs => {
                    let i = self.run_list_state.selected().unwrap_or(0);
                    if i > 0 {
                        self.run_list_state.select(Some(i - 1));
                        self.auto_preview(cmd_tx);
                    }
                }
            },
            Focus::Detail => {
                let i = self.task_list_state.selected().unwrap_or(0);
                if i > 0 {
                    self.task_list_state.select(Some(i - 1));
                    self.task_log_scroll = 0;
                }
            }
        }
    }

    fn nav_down(&mut self, cmd_tx: &mpsc::Sender<AppCommand>) {
        match self.focus {
            Focus::Sidebar => match self.sidebar_tab {
                SidebarTab::Workflows => {
                    let len = self.filtered_workflows().len();
                    if len == 0 {
                        return;
                    }
                    let i = self.workflow_list_state.selected().unwrap_or(0);
                    if i + 1 < len {
                        self.workflow_list_state.select(Some(i + 1));
                        self.auto_preview(cmd_tx);
                    }
                }
                SidebarTab::Runs => {
                    let len = self.filtered_runs().len();
                    if len == 0 {
                        return;
                    }
                    let i = self.run_list_state.selected().unwrap_or(0);
                    if i + 1 < len {
                        self.run_list_state.select(Some(i + 1));
                        self.auto_preview(cmd_tx);
                    }
                }
            },
            Focus::Detail => {
                let len = self.current_task_count();
                if len == 0 {
                    return;
                }
                match self.task_list_state.selected() {
                    None => {
                        self.task_list_state.select(Some(0));
                        self.task_log_scroll = 0;
                    }
                    Some(i) if i + 1 < len => {
                        self.task_list_state.select(Some(i + 1));
                        self.task_log_scroll = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Update selection state immediately and dispatch a background load command.
    fn auto_preview(&mut self, cmd_tx: &mpsc::Sender<AppCommand>) {
        match self.sidebar_tab {
            SidebarTab::Workflows => {
                let name = {
                    let filtered = self.filtered_workflows();
                    let Some(idx) = self.workflow_list_state.selected() else {
                        return;
                    };
                    let Some(wf) = filtered.get(idx) else { return };
                    wf.name.clone()
                };
                if self.selected_workflow.as_deref() == Some(&name) {
                    return;
                }
                self.selected_workflow = Some(name.clone());
                self.selected_run = None;
                self.run_detail = None;
                self.workflow_detail = None;
                self.task_list_state = TableState::default();
                self.task_log_scroll = 0;
                self.dag_layout = None;
                self.dag_pan_x = 0.0;
                self.dag_pan_y = 0.0;
                self.graph_mode = false;
                let _ = cmd_tx.try_send(AppCommand::LoadWorkflowDetail(name));
            }
            SidebarTab::Runs => {
                let run_id = {
                    let filtered = self.filtered_runs();
                    let Some(idx) = self.run_list_state.selected() else {
                        return;
                    };
                    let Some(run) = filtered.get(idx) else { return };
                    run.id.clone()
                };
                if self.selected_run.as_deref() == Some(&run_id) {
                    return;
                }
                self.selected_run = Some(run_id.clone());
                self.run_detail = None;
                self.task_list_state = TableState::default();
                self.task_log_scroll = 0;
                self.last_task_cursor = 0;
                self.dag_layout = None;
                self.dag_pan_x = 0.0;
                self.dag_pan_y = 0.0;
                self.graph_mode = false;
                let _ = cmd_tx.try_send(AppCommand::LoadRunDetail(run_id));
            }
        }
    }
}
