use std::cell::Cell;
use std::collections::HashMap;
use std::time::Instant;

use ratatui::{layout::Rect, widgets::TableState};

use crate::api::{Run, RunDetail, Workflow, WorkflowInfo};
use crate::dag::DagLayout;

use super::search;
use super::worker::AppUpdate;

// ── State types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum SidebarTab {
    #[default]
    Workflows,
    Runs,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Focus {
    #[default]
    Sidebar,
    Detail,
}

#[derive(Clone, Debug, Default)]
pub struct RunProgress {
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub fraction: f64,
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub should_quit: bool,
    pub is_local: bool,

    // Sidebar
    pub sidebar_tab: SidebarTab,
    pub focus: Focus,
    pub graph_mode: bool,

    // Inline search (/ key)
    pub search_query: String,
    pub is_searching: bool,

    // Command palette (Ctrl+P)
    pub command_palette: bool,
    pub cp_query: String,
    pub cp_selected: usize,

    // Workflows panel
    pub workflows: Vec<Workflow>,
    pub workflows_total: usize,
    pub workflow_list_state: TableState,
    pub selected_workflow: Option<String>,
    pub workflow_detail: Option<WorkflowInfo>,
    /// Maps workflow name → (status, optional datetime string)
    pub workflow_last_run: HashMap<String, (String, Option<String>)>,

    // Runs panel
    pub runs: Vec<Run>,
    pub runs_total: usize,
    pub run_list_state: TableState,
    pub selected_run: Option<String>,
    pub run_detail: Option<RunDetail>,
    /// Progress summary per run_id for all visible running runs.
    pub run_progress_cache: HashMap<String, RunProgress>,

    // Detail panel
    pub task_list_state: TableState,
    pub task_log_scroll: u16,
    pub last_task_cursor: usize,

    // DAG layout cache
    pub dag_layout: Option<DagLayout>,
    pub dag_pan_x: f64,
    pub dag_pan_y: f64,
    pub last_dag_area: Cell<Rect>,

    // Status bar
    pub status_msg: String,
    pub last_refresh: Instant,
}

impl Default for App {
    fn default() -> Self {
        Self::new(false)
    }
}

impl App {
    pub fn new(is_local: bool) -> Self {
        Self {
            should_quit: false,
            is_local,

            sidebar_tab: SidebarTab::Workflows,
            focus: Focus::Sidebar,
            graph_mode: false,

            search_query: String::new(),
            is_searching: false,

            command_palette: false,
            cp_query: String::new(),
            cp_selected: 0,

            workflows: Vec::new(),
            workflows_total: 0,
            workflow_list_state: TableState::default(),
            selected_workflow: None,
            workflow_detail: None,
            workflow_last_run: HashMap::new(),

            runs: Vec::new(),
            runs_total: 0,
            run_list_state: TableState::default(),
            selected_run: None,
            run_detail: None,
            run_progress_cache: HashMap::new(),

            task_list_state: TableState::default(),
            task_log_scroll: 0,
            last_task_cursor: 0,

            dag_layout: None,
            dag_pan_x: 0.0,
            dag_pan_y: 0.0,
            last_dag_area: Cell::new(Rect::default()),

            status_msg: String::from("Connecting…"),
            last_refresh: Instant::now(),
        }
    }

    // ── Apply updates from background worker ──────────────────────────────────

    pub fn apply_update(&mut self, update: AppUpdate) {
        match update {
            AppUpdate::Workflows { items, total } => {
                self.workflows_total = total;
                self.workflows = items;
                if self.selected_workflow.is_none() && !self.workflows.is_empty() {
                    self.workflow_list_state.select(Some(0));
                    self.selected_workflow = Some(self.workflows[0].name.clone());
                }
            }

            AppUpdate::Runs { items, total } => {
                self.runs_total = total;
                self.runs = items;
                // Drop cached progress for runs that are no longer running.
                let running_ids: std::collections::HashSet<&str> = self
                    .runs
                    .iter()
                    .filter(|r| r.status == "running")
                    .map(|r| r.id.as_str())
                    .collect();
                self.run_progress_cache
                    .retain(|id, _| running_ids.contains(id.as_str()));
            }

            AppUpdate::WorkflowLastRun(map) => {
                self.workflow_last_run = map;
            }

            AppUpdate::RunDetail(detail) => {
                if let Some(ref wf) = detail.workflow {
                    self.selected_workflow = Some(wf.name.clone());
                }
                self.run_detail = Some(detail);
                self.compute_dag_layout();
                self.last_refresh = Instant::now();
            }

            AppUpdate::WorkflowDetail(detail) => {
                self.workflow_detail = Some(detail);
                self.compute_dag_layout();
            }

            AppUpdate::RunProgress { run_id, progress } => {
                self.run_progress_cache.insert(run_id, progress);
            }

            AppUpdate::TriggerSucceeded { run_id } => {
                self.status_msg = format!("Triggered run {run_id}");
                self.selected_run = Some(run_id);
                self.sidebar_tab = SidebarTab::Runs;
                self.run_detail = None;
                self.task_list_state = TableState::default();
                self.task_log_scroll = 0;
                self.dag_layout = None;
            }

            AppUpdate::StatusMsg(msg) => {
                self.status_msg = msg;
                self.last_refresh = Instant::now();
            }
        }
    }

    // ── Filtered views ────────────────────────────────────────────────────────

    pub fn filtered_workflows(&self) -> Vec<&Workflow> {
        if self.search_query.is_empty() {
            return self.workflows.iter().collect();
        }
        self.workflows
            .iter()
            .filter(|wf| search::fuzzy_match(&wf.name, &self.search_query))
            .collect()
    }

    pub fn filtered_runs(&self) -> Vec<&Run> {
        if self.search_query.is_empty() {
            return self.runs.iter().collect();
        }
        self.runs
            .iter()
            .filter(|r| search::run_fuzzy_matches(r, &self.search_query))
            .collect()
    }

    /// Returns runs matching the command palette query.
    pub fn cp_results(&self) -> Vec<&Run> {
        if self.cp_query.is_empty() {
            return self.runs.iter().collect();
        }
        self.runs
            .iter()
            .filter(|r| search::run_fuzzy_matches(r, &self.cp_query))
            .collect()
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Total logical lines in the concatenated all-tasks log view (no task selected).
    pub(crate) fn all_logs_line_count(&self) -> usize {
        let Some(ref detail) = self.run_detail else {
            return 0;
        };
        detail
            .tasks
            .iter()
            .map(|t| {
                2 // header + blank separator
                + t.progress.as_ref()
                    .and_then(crate::ui::detail::parse_progress_counts)
                    .map(|_| 1)
                    .unwrap_or(0)
                + t.error.as_ref().map(|s| s.lines().count()).unwrap_or(0)
                + t.logs.as_ref().map(|s| s.lines().count()).unwrap_or(0)
                + if t.error.is_none() && t.logs.is_none() { 1 } else { 0 }
            })
            .sum()
    }

    pub(crate) fn current_task_count(&self) -> usize {
        if let Some(ref d) = self.run_detail {
            d.tasks.len()
        } else if let Some(ref d) = self.workflow_detail {
            d.tasks.len()
        } else {
            0
        }
    }

    pub(crate) fn compute_dag_layout(&mut self) {
        if let Some(ref detail) = self.run_detail {
            let names: Vec<String> = detail.tasks.iter().map(|t| t.name.clone()).collect();
            let deps: Vec<Vec<String>> =
                detail.tasks.iter().map(|t| t.depends_on.clone()).collect();
            self.dag_layout = Some(crate::dag::compute(&names, &deps));
        } else if let Some(ref detail) = self.workflow_detail {
            let names: Vec<String> = detail.tasks.iter().map(|t| t.name.clone()).collect();
            let deps: Vec<Vec<String>> =
                detail.tasks.iter().map(|t| t.depends_on.clone()).collect();
            self.dag_layout = Some(crate::dag::compute(&names, &deps));
        } else {
            self.dag_layout = None;
        }
    }

    pub fn pan_dag(&mut self, dx: f64, dy: f64) {
        let Some(ref layout) = self.dag_layout else {
            return;
        };
        let area = self.last_dag_area.get();
        let aw = area.width as f64;
        let ah = area.height as f64;
        self.dag_pan_x += dx;
        self.dag_pan_y += dy;
        let max_pan_x = (layout.total_w - aw).max(0.0);
        let max_pan_y = (layout.total_h - ah).max(0.0);
        self.dag_pan_x = self.dag_pan_x.clamp(0.0, max_pan_x);
        self.dag_pan_y = self.dag_pan_y.clamp(0.0, max_pan_y);
    }
}
