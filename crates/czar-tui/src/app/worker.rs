use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::api::{ApiClient, Run, RunDetail, Workflow, WorkflowInfo};

use super::progress::{compute_run_progress, current_time_hms};
use super::state::RunProgress;

// ── Commands (UI → worker) ─────────────────────────────────────────────────

pub enum AppCommand {
    Refresh {
        selected_workflow: Option<String>,
        selected_run: Option<String>,
    },
    ReloadWorkflowDefinitions,
    TriggerRun(String),
    CancelRun(String),
    PauseRun(String),
    ResumeRun(String),
    LoadRunDetail(String),
    LoadWorkflowDetail(String),
}

// ── Updates (worker → UI) ──────────────────────────────────────────────────

pub enum AppUpdate {
    Workflows {
        items: Vec<Workflow>,
        total: usize,
    },
    Runs {
        items: Vec<Run>,
        total: usize,
    },
    WorkflowLastRun(HashMap<String, (String, Option<String>)>),
    RunDetail(RunDetail),
    WorkflowDetail(WorkflowInfo),
    RunProgress {
        run_id: String,
        progress: RunProgress,
    },
    TriggerSucceeded {
        run_id: String,
    },
    StatusMsg(String),
}

// ── Worker ─────────────────────────────────────────────────────────────────

pub async fn run_worker(
    client: ApiClient,
    mut cmd_rx: mpsc::Receiver<AppCommand>,
    update_tx: mpsc::Sender<AppUpdate>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            AppCommand::Refresh {
                selected_workflow,
                selected_run,
            } => {
                let _ = update_tx
                    .send(AppUpdate::StatusMsg("Refreshing…".into()))
                    .await;

                match client.fetch_workflows(50, 0, None).await {
                    Ok(resp) => {
                        let _ = update_tx
                            .send(AppUpdate::Workflows {
                                items: resp.items,
                                total: resp.total,
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = update_tx
                            .send(AppUpdate::StatusMsg(format!(
                                "Error fetching workflows: {e}"
                            )))
                            .await;
                        continue;
                    }
                }

                let runs = match client.fetch_runs(50, 0, None, None).await {
                    Ok(resp) => {
                        let _ = update_tx
                            .send(AppUpdate::Runs {
                                items: resp.items.clone(),
                                total: resp.total,
                            })
                            .await;
                        resp.items
                    }
                    Err(e) => {
                        let _ = update_tx
                            .send(AppUpdate::StatusMsg(format!("Error fetching runs: {e}")))
                            .await;
                        continue;
                    }
                };

                // Progress for all running runs except the selected one (fetched below).
                for run in runs
                    .iter()
                    .filter(|r| r.status == "running" && Some(&r.id) != selected_run.as_ref())
                {
                    if let Ok(detail) = client.fetch_run_detail(&run.id).await {
                        let _ = update_tx
                            .send(AppUpdate::RunProgress {
                                run_id: run.id.clone(),
                                progress: compute_run_progress(&detail.tasks),
                            })
                            .await;
                    }
                }

                // Workflow last-run metadata.
                let workflow_names: Vec<String> = runs
                    .iter()
                    .map(|r| r.workflow.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                let mut last_run_map: HashMap<String, (String, Option<String>)> = HashMap::new();
                for name in &workflow_names {
                    if let Ok(resp) = client.fetch_runs(1, 0, Some(name), None).await
                        && let Some(run) = resp.items.into_iter().next()
                    {
                        let dt = run.started_at.or(Some(run.created_at));
                        last_run_map.insert(name.clone(), (run.status, dt));
                    }
                }
                let _ = update_tx
                    .send(AppUpdate::WorkflowLastRun(last_run_map))
                    .await;

                if let Some(name) = selected_workflow
                    && let Ok(detail) = client.fetch_workflow_detail(&name).await
                {
                    let _ = update_tx.send(AppUpdate::WorkflowDetail(detail)).await;
                }

                if let Some(run_id) = selected_run
                    && let Ok(detail) = client.fetch_run_detail(&run_id).await
                {
                    let _ = update_tx
                        .send(AppUpdate::RunProgress {
                            run_id: run_id.clone(),
                            progress: compute_run_progress(&detail.tasks),
                        })
                        .await;
                    let _ = update_tx.send(AppUpdate::RunDetail(detail)).await;
                }

                let _ = update_tx
                    .send(AppUpdate::StatusMsg(format!(
                        "Refreshed at {}",
                        current_time_hms()
                    )))
                    .await;
            }

            AppCommand::LoadRunDetail(run_id) => match client.fetch_run_detail(&run_id).await {
                Ok(detail) => {
                    let _ = update_tx
                        .send(AppUpdate::RunProgress {
                            run_id: run_id.clone(),
                            progress: compute_run_progress(&detail.tasks),
                        })
                        .await;
                    let _ = update_tx.send(AppUpdate::RunDetail(detail)).await;
                }
                Err(e) => {
                    let _ = update_tx
                        .send(AppUpdate::StatusMsg(format!("Error loading run: {e}")))
                        .await;
                }
            },

            AppCommand::LoadWorkflowDetail(name) => {
                match client.fetch_workflow_detail(&name).await {
                    Ok(detail) => {
                        let _ = update_tx.send(AppUpdate::WorkflowDetail(detail)).await;
                    }
                    Err(e) => {
                        let _ = update_tx
                            .send(AppUpdate::StatusMsg(format!("Error loading workflow: {e}")))
                            .await;
                    }
                }
            }

            AppCommand::TriggerRun(workflow) => match client.trigger_run(&workflow).await {
                Ok(resp) => {
                    let _ = update_tx
                        .send(AppUpdate::TriggerSucceeded {
                            run_id: resp.run_id,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = update_tx
                        .send(AppUpdate::StatusMsg(format!("Trigger failed: {e}")))
                        .await;
                }
            },

            AppCommand::CancelRun(run_id) => match client.cancel_run(&run_id).await {
                Ok(()) => {
                    let _ = update_tx
                        .send(AppUpdate::StatusMsg(format!("Cancelled run {run_id}")))
                        .await;
                    if let Ok(detail) = client.fetch_run_detail(&run_id).await {
                        let _ = update_tx.send(AppUpdate::RunDetail(detail)).await;
                    }
                }
                Err(e) => {
                    let _ = update_tx
                        .send(AppUpdate::StatusMsg(format!("Cancel failed: {e}")))
                        .await;
                }
            },

            AppCommand::PauseRun(run_id) => match client.pause_run(&run_id).await {
                Ok(()) => {
                    let _ = update_tx
                        .send(AppUpdate::StatusMsg(format!("Paused run {run_id}")))
                        .await;
                    if let Ok(detail) = client.fetch_run_detail(&run_id).await {
                        let _ = update_tx.send(AppUpdate::RunDetail(detail)).await;
                    }
                }
                Err(e) => {
                    let _ = update_tx
                        .send(AppUpdate::StatusMsg(format!("Pause failed: {e}")))
                        .await;
                }
            },

            AppCommand::ResumeRun(run_id) => match client.resume_run(&run_id).await {
                Ok(()) => {
                    let _ = update_tx
                        .send(AppUpdate::StatusMsg(format!("Resumed run {run_id}")))
                        .await;
                    if let Ok(detail) = client.fetch_run_detail(&run_id).await {
                        let _ = update_tx.send(AppUpdate::RunDetail(detail)).await;
                    }
                }
                Err(e) => {
                    let _ = update_tx
                        .send(AppUpdate::StatusMsg(format!("Resume failed: {e}")))
                        .await;
                }
            },

            AppCommand::ReloadWorkflowDefinitions => {
                match client.reload_workflow_definitions().await {
                    Ok(resp) => {
                        let reloaded = resp.get("reloaded").and_then(|v| v.as_u64()).unwrap_or(0);
                        let failed = resp.get("failed").and_then(|v| v.as_u64()).unwrap_or(0);
                        let _ = update_tx
                            .send(AppUpdate::StatusMsg(format!(
                                "Reloaded {reloaded} definition(s), {failed} failed"
                            )))
                            .await;
                    }
                    Err(e) => {
                        let _ = update_tx
                            .send(AppUpdate::StatusMsg(format!("Reload failed: {e}")))
                            .await;
                    }
                }
            }
        }
    }
}
