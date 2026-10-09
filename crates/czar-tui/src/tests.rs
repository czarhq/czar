use axum::response::IntoResponse;
use ratatui::{Terminal, backend::TestBackend};

use crate::api::{ApiClient, Run, RunDetail, Task, Workflow, WorkflowInfo, WorkflowTaskInfo};
use crate::app::progress::compute_run_progress;
use crate::app::search::{fuzzy_match, fuzzy_match_positions};
use crate::app::worker::AppUpdate;
use crate::app::{App, RunProgress};
use crate::ui;

fn make_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(width, height)).unwrap()
}

fn make_workflow(name: &str) -> Workflow {
    Workflow {
        name: name.to_string(),
        schedule: None,
        schedule_enabled: false,
        next_scheduled_at: None,
    }
}

fn make_run(id: &str, workflow: &str, status: &str) -> Run {
    Run {
        id: id.to_string(),
        workflow: workflow.to_string(),
        status: status.to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        started_at: None,
        finished_at: None,
    }
}

fn make_task(name: &str, status: &str) -> Task {
    Task {
        id: format!("task-{name}"),
        name: name.to_string(),
        status: status.to_string(),
        executor: "python".to_string(),
        depends_on: vec![],
        started_at: None,
        finished_at: None,
        dispatched_at: None,
        output: None,
        logs: None,
        error: None,
        progress: None,
    }
}

// ── App state tests ───────────────────────────────────────────────────────────

#[test]
fn app_new_initial_state() {
    let app = App::new(true);
    assert!(!app.should_quit);
    assert!(app.workflows.is_empty());
    assert!(app.runs.is_empty());
    assert!(app.selected_workflow.is_none());
    assert!(app.selected_run.is_none());
    assert!(app.run_detail.is_none());
}

#[test]
fn apply_update_workflows_selects_first() {
    let mut app = App::new(true);
    app.apply_update(AppUpdate::Workflows {
        items: vec![make_workflow("alpha"), make_workflow("beta")],
        total: 2,
    });
    assert_eq!(app.workflows.len(), 2);
    assert_eq!(app.selected_workflow.as_deref(), Some("alpha"));
    assert_eq!(app.workflow_list_state.selected(), Some(0));
}

#[test]
fn apply_update_workflows_preserves_selection() {
    let mut app = App::new(true);
    app.selected_workflow = Some("beta".to_string());
    app.apply_update(AppUpdate::Workflows {
        items: vec![make_workflow("alpha"), make_workflow("beta")],
        total: 2,
    });
    // Should not overwrite existing selection.
    assert_eq!(app.selected_workflow.as_deref(), Some("beta"));
}

#[test]
fn apply_update_runs_clears_stale_progress() {
    let mut app = App::new(true);
    app.run_progress_cache.insert(
        "run-1".to_string(),
        RunProgress {
            total: 5,
            done: 2,
            failed: 0,
            fraction: 0.4,
        },
    );
    // run-1 is no longer in the running list.
    app.apply_update(AppUpdate::Runs {
        items: vec![],
        total: 0,
    });
    assert!(app.run_progress_cache.is_empty());
}

// ── Render smoke tests ────────────────────────────────────────────────────────

#[test]
fn render_empty_app_no_panic() {
    let mut terminal = make_terminal(80, 24);
    let app = App::new(true);
    terminal.draw(|f| ui::render(f, &app)).unwrap();
}

#[test]
fn render_with_workflows_shows_name() {
    let mut terminal = make_terminal(80, 24);
    let mut app = App::new(true);
    app.apply_update(AppUpdate::Workflows {
        items: vec![make_workflow("my-workflow")],
        total: 1,
    });
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let buf = terminal.backend().buffer().clone();
    let content: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(
        content.contains("my-workflow"),
        "workflow name should appear in render output"
    );
}

#[test]
fn render_with_run_and_tasks_no_panic() {
    let mut terminal = make_terminal(120, 40);
    let mut app = App::new(true);
    let run = make_run("aaaabbbbccccdddd", "my-workflow", "running");
    let tasks = vec![
        make_task("ingest", "success"),
        make_task("process", "running"),
        make_task("export", "pending"),
    ];
    app.apply_update(AppUpdate::Runs {
        items: vec![run.clone()],
        total: 1,
    });
    app.apply_update(AppUpdate::RunDetail(RunDetail {
        run: run.clone(),
        tasks,
        workflow: None,
    }));
    app.selected_run = Some(run.id.clone());
    terminal.draw(|f| ui::render(f, &app)).unwrap();
}

#[test]
fn render_with_task_logs_no_panic() {
    let mut terminal = make_terminal(120, 40);
    let mut app = App::new(true);
    let run = make_run("aaaabbbbccccdddd", "my-workflow", "success");
    let mut task = make_task("process", "success");
    task.logs = Some("line 1\nline 2\nline 3".to_string());
    app.apply_update(AppUpdate::RunDetail(RunDetail {
        run: run.clone(),
        tasks: vec![task],
        workflow: None,
    }));
    app.selected_run = Some(run.id.clone());
    app.task_list_state.select(Some(0));
    terminal.draw(|f| ui::render(f, &app)).unwrap();
}

#[test]
fn render_with_task_progress_no_panic() {
    let mut terminal = make_terminal(120, 40);
    let mut app = App::new(true);
    let run = make_run("aaaabbbbccccdddd", "my-workflow", "running");
    let mut task = make_task("process", "running");
    task.progress = Some(serde_json::json!({"current": 30, "total": 100, "message": "processing"}));
    app.apply_update(AppUpdate::RunDetail(RunDetail {
        run: run.clone(),
        tasks: vec![task],
        workflow: None,
    }));
    app.selected_run = Some(run.id.clone());
    app.task_list_state.select(Some(0));
    terminal.draw(|f| ui::render(f, &app)).unwrap();
}

#[test]
fn render_workflow_detail_no_panic() {
    let mut terminal = make_terminal(120, 40);
    let mut app = App::new(true);
    app.apply_update(AppUpdate::WorkflowDetail(WorkflowInfo {
        name: "my-workflow".to_string(),
        tasks: vec![
            WorkflowTaskInfo {
                name: "step-a".to_string(),
                depends_on: vec![],
                executor: "python".to_string(),
            },
            WorkflowTaskInfo {
                name: "step-b".to_string(),
                depends_on: vec!["step-a".to_string()],
                executor: "python".to_string(),
            },
        ],
        flows: vec![],
        schedule: None,
        schedule_enabled: false,
        next_scheduled_at: None,
    }));
    app.selected_workflow = Some("my-workflow".to_string());
    terminal.draw(|f| ui::render(f, &app)).unwrap();
}

#[test]
fn render_graph_mode_no_panic() {
    let mut terminal = make_terminal(120, 40);
    let mut app = App::new(true);
    let run = make_run("aaaabbbbccccdddd", "my-workflow", "running");
    app.apply_update(AppUpdate::RunDetail(RunDetail {
        run: run.clone(),
        tasks: vec![make_task("a", "success"), make_task("b", "running")],
        workflow: None,
    }));
    app.selected_run = Some(run.id.clone());
    app.graph_mode = true;
    terminal.draw(|f| ui::render(f, &app)).unwrap();
}

// ── compute_run_progress ──────────────────────────────────────────────────────

#[test]
fn run_progress_empty() {
    let p = compute_run_progress(&[]);
    assert_eq!(p.total, 0);
    assert_eq!(p.done, 0);
    assert_eq!(p.fraction, 0.0);
}

#[test]
fn run_progress_all_success() {
    let tasks = vec![make_task("a", "success"), make_task("b", "success")];
    let p = compute_run_progress(&tasks);
    assert_eq!(p.total, 2);
    assert_eq!(p.done, 2);
    assert_eq!(p.failed, 0);
    assert!((p.fraction - 1.0).abs() < 1e-9);
}

#[test]
fn run_progress_with_failure() {
    let tasks = vec![make_task("a", "success"), make_task("b", "failed")];
    let p = compute_run_progress(&tasks);
    assert_eq!(p.done, 2);
    assert_eq!(p.failed, 1);
}

#[test]
fn run_progress_fractional_running() {
    let mut running = make_task("a", "running");
    running.progress = Some(serde_json::json!({"current": 50, "total": 100}));
    let tasks = vec![make_task("b", "success"), running];
    let p = compute_run_progress(&tasks);
    // done=1 (1.0) + running with 50% (0.5) = 1.5 / 2 = 0.75
    assert!((p.fraction - 0.75).abs() < 1e-9);
}

// ── fuzzy_match ───────────────────────────────────────────────────────────────

#[test]
fn fuzzy_match_empty_query_always_true() {
    assert!(fuzzy_match("anything", ""));
    assert!(fuzzy_match("", ""));
}

#[test]
fn fuzzy_match_subsequence() {
    assert!(fuzzy_match("my-workflow", "mywf"));
    assert!(fuzzy_match("my-workflow", "MY-WF"));
    assert!(!fuzzy_match("abc", "abd"));
}

#[test]
fn fuzzy_match_positions_returns_indices() {
    // "my-workflow": m=0, w=3, f=7
    let positions = fuzzy_match_positions("my-workflow", "mwf").unwrap();
    assert_eq!(positions, vec![0, 3, 7]);
}

#[test]
fn fuzzy_match_positions_no_match_returns_none() {
    assert!(fuzzy_match_positions("abc", "xyz").is_none());
}

#[tokio::test]
async fn client_sends_the_api_token() {
    let app = axum::Router::new().route(
        "/api/runs",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            if headers
                .get("authorization")
                .is_some_and(|v| v == "Bearer s3cret")
            {
                axum::Json(serde_json::json!({"items": [], "total": 0})).into_response()
            } else {
                axum::http::StatusCode::UNAUTHORIZED.into_response()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, app).await });

    let anonymous = ApiClient::new(&format!("http://{addr}"));
    let err = anonymous.fetch_runs(10, 0, None, None).await.unwrap_err();
    assert!(err.to_string().contains("401"), "{err}");
    let client = anonymous.with_token(Some("s3cret".to_string()));
    assert_eq!(
        client
            .fetch_runs(10, 0, None, None)
            .await
            .expect("runs")
            .total,
        0
    );
}
