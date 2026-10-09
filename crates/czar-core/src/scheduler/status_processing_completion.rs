use super::*;
use crate::database::NewWorkflowTask;
use crate::models::json_inner;
use crate::models::{RunStatus, TaskStatus};
use crate::notifications::{RunFailureNotification, send_run_failure_notifications};
use crate::workflow::workflow_validation_helpers::validate_value_against_type_schema;
use std::collections::BTreeMap;

impl<D: Database + 'static, E: ExecutorManager + 'static> Scheduler<D, E> {
    pub(super) async fn validate_task_output_contract(
        &self,
        task_id: Uuid,
        output: Option<&serde_json::Value>,
    ) -> Result<Option<String>> {
        let Some(signature) = self.load_task_signature(task_id).await? else {
            return Ok(None);
        };
        let Some(output_type) = signature.get("output_type") else {
            return Ok(None);
        };

        let aliases = signature
            .get("type_aliases")
            .and_then(|value| value.as_object())
            .map(|map| {
                map.iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();

        let Some(output) = output else {
            return Ok(Some(
                "output validation failed: task declared output_type but returned no output"
                    .to_string(),
            ));
        };

        match validate_value_against_type_schema(output, output_type, &aliases) {
            Ok(()) => Ok(None),
            Err(reason) => Ok(Some(format!("output validation failed: {reason}"))),
        }
    }

    async fn load_task_signature(&self, task_id: Uuid) -> Result<Option<serde_json::Value>> {
        let (run_id, task_name) = self.get_task_identity_cached(task_id).await?;
        let (workflow_id, snapshot_id_opt) = self.get_run_meta_cached(run_id).await?;

        if let Some(snapshot_id) = snapshot_id_opt {
            let snapshot = self.get_snapshot_cached(snapshot_id).await?;
            let tasks_json = json_inner(&snapshot.tasks_json);
            let snapshot_tasks: Vec<NewWorkflowTask> = serde_json::from_value(tasks_json.clone())?;
            return Ok(snapshot_tasks
                .into_iter()
                .find(|task| task.task_name == task_name)
                .and_then(|task| task.signature));
        }

        let workflow_tasks = self.db.list_workflow_tasks(workflow_id).await?;
        Ok(workflow_tasks
            .into_iter()
            .find(|task| task.task_name == task_name)
            .and_then(|task| {
                task.signature
                    .map(|signature| json_inner(&signature).clone())
            }))
    }

    pub(super) async fn check_run_completions_batch(
        &self,
        run_ids: impl IntoIterator<Item = Uuid>,
    ) -> Result<()> {
        let run_ids: Vec<Uuid> = run_ids.into_iter().collect();
        if run_ids.is_empty() {
            return Ok(());
        }

        let stats_map = self.db.batch_get_run_task_stats(&run_ids).await?;

        let complete_runs: Vec<(Uuid, i64, i64, i64)> = stats_map
            .into_iter()
            .filter(|(_, (total, completed, _))| completed == total)
            .map(|(run_id, (total, completed, failed))| (run_id, total, completed, failed))
            .collect();

        if complete_runs.is_empty() {
            return Ok(());
        }

        let status_updates: Vec<(Uuid, RunStatus, Option<String>)> = complete_runs
            .iter()
            .map(|(run_id, _, _, failed)| {
                let status = if *failed > 0 {
                    RunStatus::Failed
                } else {
                    RunStatus::Success
                };
                (*run_id, status, None)
            })
            .collect();

        self.db.batch_update_run_statuses(&status_updates).await?;

        for (run_id, total, completed, failed) in complete_runs {
            let new_status = if failed > 0 {
                RunStatus::Failed
            } else {
                RunStatus::Success
            };
            info!(
                "Run {} completed with status: {} ({}/{} tasks)",
                run_id,
                new_status.as_str(),
                completed,
                total
            );

            if let Ok((workflow_id, snapshot_id)) = self.get_run_meta_cached(run_id).await {
                self.evict_run_meta(run_id);
                let event = match new_status {
                    RunStatus::Success => "run.success",
                    RunStatus::Failed => "run.failed",
                    _ => continue,
                };
                let failed_tasks = if new_status == RunStatus::Failed {
                    match self.db.list_tasks(run_id).await {
                        Ok(tasks) => Some(
                            tasks
                                .into_iter()
                                .filter(|t| matches!(t.status, TaskStatus::Failed))
                                .map(|t| t.task_name)
                                .collect::<Vec<_>>(),
                        ),
                        Err(_) => None,
                    }
                } else {
                    None
                };
                self.fire_run_hooks(event, run_id, snapshot_id, &workflow_id, None, failed_tasks)
                    .await;

                if new_status == RunStatus::Failed && !self.config.notifications.is_empty() {
                    let workflow_name = self
                        .db
                        .get_workflow_by_id(workflow_id)
                        .await
                        .map(|workflow| workflow.name)
                        .unwrap_or_else(|_| workflow_id.to_string());
                    let notification = RunFailureNotification {
                        run_id: run_id.to_string(),
                        workflow_name,
                        error: None,
                        failed_tasks: match self.db.list_tasks(run_id).await {
                            Ok(tasks) => tasks
                                .into_iter()
                                .filter(|t| matches!(t.status, TaskStatus::Failed))
                                .map(|t| t.task_name)
                                .collect(),
                            Err(_) => Vec::new(),
                        },
                    };
                    if let Err(err) =
                        send_run_failure_notifications(&self.config.notifications, &notification)
                            .await
                    {
                        error!("Failed to send built-in failure notifications: {}", err);
                    }
                }
            }
        }

        Ok(())
    }

    pub(super) async fn check_run_completion(&self, run_id: Uuid) -> Result<()> {
        let (total, completed, failed) = self.db.get_run_task_stats(run_id).await?;

        if completed == total {
            let new_status = if failed > 0 {
                RunStatus::Failed
            } else {
                RunStatus::Success
            };

            self.db.update_run_status(run_id, new_status, None).await?;
            info!(
                "Run {} completed with status: {} ({}/{} tasks)",
                run_id,
                new_status.as_str(),
                completed,
                total
            );

            if let Ok((workflow_id, snapshot_id)) = self.get_run_meta_cached(run_id).await {
                // Evict run from meta cache once we have what we need.
                self.evict_run_meta(run_id);
                let event = match new_status {
                    RunStatus::Success => "run.success",
                    RunStatus::Failed => "run.failed",
                    _ => return Ok(()),
                };
                let failed_tasks = if new_status == RunStatus::Failed {
                    match self.db.list_tasks(run_id).await {
                        Ok(tasks) => Some(
                            tasks
                                .into_iter()
                                .filter(|t| matches!(t.status, crate::models::TaskStatus::Failed))
                                .map(|t| t.task_name)
                                .collect::<Vec<_>>(),
                        ),
                        Err(_) => None,
                    }
                } else {
                    None
                };
                self.fire_run_hooks(event, run_id, snapshot_id, &workflow_id, None, failed_tasks)
                    .await;

                if new_status == RunStatus::Failed && !self.config.notifications.is_empty() {
                    let workflow_name = self
                        .db
                        .get_workflow_by_id(workflow_id)
                        .await
                        .map(|workflow| workflow.name)
                        .unwrap_or_else(|_| workflow_id.to_string());
                    let notification = RunFailureNotification {
                        run_id: run_id.to_string(),
                        workflow_name,
                        error: None,
                        failed_tasks: match self.db.list_tasks(run_id).await {
                            Ok(tasks) => tasks
                                .into_iter()
                                .filter(|t| matches!(t.status, crate::models::TaskStatus::Failed))
                                .map(|t| t.task_name)
                                .collect(),
                            Err(_) => Vec::new(),
                        },
                    };
                    if let Err(err) =
                        send_run_failure_notifications(&self.config.notifications, &notification)
                            .await
                    {
                        error!("Failed to send built-in failure notifications: {}", err);
                    }
                }
            }
        }

        Ok(())
    }

    pub(super) async fn process_job_completions(
        &self,
        completions: Vec<JobCompletionNotification>,
    ) {
        for completion in completions {
            info!(
                "Processing deferred job completion for task {}: success={}",
                completion.task_id, completion.success
            );
            let completion_error = completion.error.clone();

            if completion.success {
                if let Err(e) = self
                    .db
                    .update_task_status(completion.task_id, TaskStatus::Success, None, None)
                    .await
                {
                    error!(
                        "Failed to mark task {} as successful: {}",
                        completion.task_id, e
                    );
                    continue;
                }
            } else {
                let error_msg = completion_error
                    .clone()
                    .unwrap_or_else(|| "Deferred job failed".to_string());
                if let Err(e) = self
                    .db
                    .update_task_status(
                        completion.task_id,
                        TaskStatus::Failed,
                        None,
                        Some(&error_msg),
                    )
                    .await
                {
                    error!(
                        "Failed to mark task {} as failed: {}",
                        completion.task_id, e
                    );
                    continue;
                }
            }

            if let Ok((run_id, task_name)) = self.get_task_identity_cached(completion.task_id).await
            {
                if let Ok((workflow_id, _snapshot_id)) = self.get_run_meta_cached(run_id).await {
                    let task_status = if completion.success {
                        TaskStatus::Success
                    } else {
                        TaskStatus::Failed
                    };
                    let error_ref = completion_error.as_deref();
                    if let Err(err) = self
                        .db
                        .upsert_workflow_task_state(
                            workflow_id,
                            &task_name,
                            task_status,
                            run_id,
                            completion.task_id,
                            None,
                            error_ref,
                            Some(Utc::now()),
                        )
                        .await
                    {
                        error!(
                            "Failed to persist task state for deferred completion {} / '{}': {}",
                            run_id, task_name, err
                        );
                    }
                }

                if let Err(e) = self.check_run_completion(run_id).await {
                    error!("Error checking run completion: {}", e);
                }
            }
        }
    }
}
