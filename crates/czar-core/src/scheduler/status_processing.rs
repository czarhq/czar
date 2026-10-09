use super::*;
use crate::database::WorkflowTaskStateEntry;
use crate::models::TaskStatus;

impl<D: Database + 'static, E: ExecutorManager + 'static> Scheduler<D, E> {
    pub(super) async fn process_status_updates(&self, updates: Vec<StatusUpdate>) -> Result<()> {
        let mut task_updates: Vec<(Uuid, TaskStatus, Option<String>, Option<String>)> = Vec::new();
        let mut runs_to_check = std::collections::HashSet::new();
        let mut failed_by_run: HashMap<Uuid, Vec<String>> = HashMap::new();
        // Accumulate log text per task; flushed in a single call per task at the end.
        let mut log_buffer: HashMap<Uuid, String> = HashMap::new();
        // Accumulate workflow_task_state upserts; flushed in a single batch call at the end.
        let mut task_state_upserts: Vec<WorkflowTaskStateEntry> = Vec::new();
        let failed_ids: Vec<Uuid> = updates
            .iter()
            .filter(|u| matches!(TaskStatus::parse(&u.status), Some(TaskStatus::Failed)))
            .map(|u| u.task_id)
            .collect();
        let retry_meta = if failed_ids.is_empty() {
            HashMap::new()
        } else {
            self.db.get_task_retry_meta(&failed_ids).await?
        };

        for update in &updates {
            // Handle in-task progress updates: store progress and continue, don't change task status
            if update.status == "progress" {
                if let Some(progress) = update.progress.as_ref()
                    && let Err(e) = self
                        .db
                        .update_task_progress(update.task_id, progress.clone())
                        .await
                {
                    error!(
                        "Failed to update progress for task {}: {}",
                        update.task_id, e
                    );
                }
                continue;
            }

            if let Some(log) = update.log.as_ref() {
                log_buffer.entry(update.task_id).or_default().push_str(log);
            }
            let Some(mut effective_status) = TaskStatus::parse(&update.status) else {
                continue;
            };
            let mut effective_output = update.output.clone();
            let mut effective_error = update.error.clone();

            if let Some(output) = effective_output.as_ref()
                && let Some(deferred) = output.get("deferred").and_then(|v| v.as_array())
            {
                info!(
                    "Task {} returned {} deferrables",
                    update.task_id,
                    deferred.len()
                );

                for deferrable_data in deferred {
                    if let (Some(service_type), Some(job_id)) = (
                        deferrable_data.get("service_type").and_then(|v| v.as_str()),
                        deferrable_data.get("job_id").and_then(|v| v.as_str()),
                    ) {
                        match self
                            .db
                            .create_deferred_job(
                                update.task_id,
                                service_type,
                                job_id,
                                deferrable_data.clone(),
                            )
                            .await
                        {
                            Ok(_) => {
                                info!(
                                    "Created deferred job for task {}: {} ({})",
                                    update.task_id, job_id, service_type
                                );
                            }
                            Err(e) => {
                                error!(
                                    "Failed to create deferred job for task {}: {}",
                                    update.task_id, e
                                );
                            }
                        }
                    } else {
                        error!(
                            "Invalid deferrable data for task {}: missing service_type or job_id",
                            update.task_id
                        );
                    }
                }

                continue;
            }

            // Handle task.skipped hooks before filtering out skipped status
            if matches!(effective_status, TaskStatus::Skipped)
                && let Ok((run_id, task_name)) = self.get_task_identity_cached(update.task_id).await
                && let Ok((workflow_id, snapshot_id)) = self.get_run_meta_cached(run_id).await
            {
                self.fire_task_hooks(
                    "task.skipped",
                    run_id,
                    snapshot_id,
                    &workflow_id,
                    &task_name,
                    None,
                    None,
                )
                .await;
            }
            if !matches!(
                effective_status,
                TaskStatus::Success
                    | TaskStatus::Failed
                    | TaskStatus::Running
                    | TaskStatus::Skipped
            ) {
                continue;
            }

            if matches!(effective_status, TaskStatus::Success)
                && let Some(validation_error) = self
                    .validate_task_output_contract(update.task_id, effective_output.as_ref())
                    .await?
            {
                effective_status = TaskStatus::Failed;
                effective_output = None;
                effective_error = Some(validation_error);
            }

            if let Some(output) = effective_output.as_ref()
                && let Err(e) = self
                    .db
                    .update_task_output(update.task_id, output.clone())
                    .await
            {
                error!("Failed to store output for task {}: {}", update.task_id, e);
            }

            if matches!(effective_status, TaskStatus::Failed)
                && let Some(error) = effective_error.as_deref()
                && !error.trim().is_empty()
            {
                let ts = Utc::now().format("%H:%M:%S%.3f");
                let error_log = error
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| format!("E {ts} {line}\n"))
                    .collect::<String>();
                if !error_log.is_empty() {
                    log_buffer
                        .entry(update.task_id)
                        .or_default()
                        .push_str(&error_log);
                }
            }

            if matches!(effective_status, TaskStatus::Failed) {
                let retry_settings = retry_meta.get(&update.task_id).cloned().unwrap_or(
                    crate::database::TaskRetryMeta {
                        attempts: 0,
                        max_retries: 0,
                        retry_delay_seconds: None,
                    },
                );
                let attempts = retry_settings.attempts;
                let max_retries = retry_settings.max_retries;
                if attempts < max_retries {
                    let error = effective_error.as_deref().unwrap_or("retrying");
                    let backoff = retry_settings
                        .retry_delay_seconds
                        .map(|value| value.max(0) as u64)
                        .unwrap_or_else(|| retry_backoff_seconds(attempts + 1));
                    let retry_at = Utc::now()
                        + chrono::Duration::seconds(i64::try_from(backoff).unwrap_or(i64::MAX));
                    if let Err(e) = self
                        .db
                        .reset_task_for_retry(update.task_id, Some(error), Some(retry_at))
                        .await
                    {
                        error!("Failed to reset task {} for retry: {}", update.task_id, e);
                        task_updates.push((
                            update.task_id,
                            effective_status,
                            None,
                            effective_error.clone(),
                        ));
                    } else {
                        if let Ok((_, task_name)) =
                            self.get_task_identity_cached(update.task_id).await
                        {
                            let next_attempt = attempts + 1;
                            let sys_log = format!(
                                "S {task_name} failed, retrying (attempt {next_attempt} of {max_retries})\n"
                            );
                            log_buffer
                                .entry(update.task_id)
                                .or_default()
                                .push_str(&sys_log);
                        }
                        continue;
                    }
                }
            }

            task_updates.push((
                update.task_id,
                effective_status,
                None,
                effective_error.clone(),
            ));

            // Fire task.start hook when a task begins running
            if matches!(effective_status, TaskStatus::Running)
                && let Ok((run_id, task_name)) = self.get_task_identity_cached(update.task_id).await
                && let Ok((workflow_id, snapshot_id)) = self.get_run_meta_cached(run_id).await
            {
                self.fire_task_hooks(
                    "task.start",
                    run_id,
                    snapshot_id,
                    &workflow_id,
                    &task_name,
                    None,
                    None,
                )
                .await;
            }

            if matches!(
                effective_status,
                TaskStatus::Success | TaskStatus::Failed | TaskStatus::Skipped
            ) {
                let (run_id, task_name) = self.get_task_identity_cached(update.task_id).await?;
                runs_to_check.insert(run_id);
                match self.get_run_meta_cached(run_id).await {
                    Ok((workflow_id, snapshot_id)) => {
                        task_state_upserts.push(WorkflowTaskStateEntry {
                            workflow_id,
                            task_name: task_name.clone(),
                            last_status: effective_status,
                            last_run_id: run_id,
                            last_task_id: update.task_id,
                            last_output: effective_output.clone(),
                            last_error: effective_error.clone(),
                            last_finished_at: Some(Utc::now()),
                        });
                        // Fire task-level hooks
                        let hook_event = match effective_status {
                            TaskStatus::Success => Some("task.success"),
                            TaskStatus::Failed => Some("task.failed"),
                            _ => None,
                        };
                        if let Some(event) = hook_event {
                            self.fire_task_hooks(
                                event,
                                run_id,
                                snapshot_id,
                                &workflow_id,
                                &task_name,
                                effective_error.as_deref(),
                                effective_output.clone(),
                            )
                            .await;
                        }
                    }
                    Err(err) => {
                        error!(
                            "Failed to load run meta for run {} while updating task state for '{}': {}",
                            run_id, task_name, err
                        );
                    }
                }
                // Evict from cache once the task has reached a terminal state.
                self.evict_task_identity(update.task_id);
                if matches!(effective_status, TaskStatus::Failed) {
                    failed_by_run.entry(run_id).or_default().push(task_name);
                }
            }
        }

        if !task_updates.is_empty() {
            // Deduplicate by task_id: if the same task appears with both a transient
            // (Running) and a terminal (Success/Failed/Cancelled) status in one batch,
            // keep only the terminal status.  Duplicates arise when fast tasks complete
            // before the scheduler drains the channel — the unnest UPDATE's WHERE clause
            // evaluates against the original row, making duplicate-key behavior undefined.
            let mut deduped: HashMap<Uuid, (Uuid, TaskStatus, Option<String>, Option<String>)> =
                HashMap::new();
            for (task_id, status, execution_name, error) in &task_updates {
                let is_terminal = matches!(
                    status,
                    TaskStatus::Success | TaskStatus::Failed | TaskStatus::Cancelled
                );
                let entry = deduped.entry(*task_id);
                match entry {
                    std::collections::hash_map::Entry::Occupied(mut o) => {
                        let existing_terminal = matches!(
                            o.get().1,
                            TaskStatus::Success | TaskStatus::Failed | TaskStatus::Cancelled
                        );
                        // Only overwrite if the new status is terminal, or the existing one isn't.
                        if is_terminal || !existing_terminal {
                            o.insert((*task_id, *status, execution_name.clone(), error.clone()));
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(v) => {
                        v.insert((*task_id, *status, execution_name.clone(), error.clone()));
                    }
                }
            }
            let borrowed_updates = deduped
                .values()
                .map(|(task_id, status, execution_name, error)| {
                    (
                        *task_id,
                        *status,
                        execution_name.as_deref(),
                        error.as_deref(),
                    )
                })
                .collect::<Vec<_>>();
            self.db.batch_update_task_status(&borrowed_updates).await?;
            info!(
                "Processed {} status updates from executors",
                task_updates.len()
            );
        }

        for (run_id, names) in failed_by_run {
            let mut pending = names;
            let mut seen = HashSet::new();
            while !pending.is_empty() {
                let next = self
                    .db
                    .mark_tasks_failed_by_dependency(run_id, &pending, "upstream failed")
                    .await?;
                pending = next
                    .into_iter()
                    .filter(|name| seen.insert(name.clone()))
                    .collect();
            }
            runs_to_check.insert(run_id);
        }

        // Run the three independent post-processing writes concurrently to save
        // one DB round-trip per call under high-latency connections.
        let completions_fut = self.check_run_completions_batch(runs_to_check);

        let states_fut = async {
            if !task_state_upserts.is_empty()
                && let Err(e) = self
                    .db
                    .batch_upsert_workflow_task_states(&task_state_upserts)
                    .await
            {
                error!("Failed to batch upsert workflow task states: {}", e);
            }
        };

        let logs_fut = async {
            if !log_buffer.is_empty() {
                let entries: Vec<(Uuid, String)> = log_buffer.into_iter().collect();
                if let Err(e) = self.db.batch_append_task_logs(&entries).await {
                    error!("Failed to batch append logs: {}", e);
                }
            }
        };

        let (completion_result, _, _) = tokio::join!(completions_fut, states_fut, logs_fut);
        completion_result?;

        Ok(())
    }
    pub(super) async fn enforce_timeouts(&self) -> Result<()> {
        let running = self.db.get_running_tasks().await?;
        if running.is_empty() {
            return Ok(());
        }

        let now = Utc::now();
        let mut updates = Vec::new();
        for task in running {
            let timeout = match task.timeout_seconds {
                Some(value) if value > 0 => value,
                _ => continue,
            };
            let started = task
                .started_at
                .or(task.dispatched_at)
                .unwrap_or(task.created_at);
            let elapsed = now.signed_duration_since(started).num_seconds();
            if elapsed > i64::from(timeout) {
                updates.push(StatusUpdate {
                    task_id: task.id,
                    status: "failed".to_string(),
                    log: None,
                    output: None,
                    error: Some(format!("timeout after {}s", timeout)),
                    progress: None,
                });
            }
        }

        if !updates.is_empty() {
            self.process_status_updates(updates).await?;
        }

        Ok(())
    }
}
