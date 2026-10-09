use super::*;
use crate::models::json_inner;
use crate::models::{RunStatus, TaskStatus};
use crate::task_execution::TaskOutcome;

fn task_parallel_group_limit(task: &crate::models::TaskWithWorkflow) -> Option<(String, usize)> {
    let params = task.params.as_ref().map(json_inner)?;
    let params_obj = params.as_object()?;
    let group = params_obj.get("parallel_group")?.as_str()?.trim();
    if group.is_empty() {
        return None;
    }
    let concurrency = params_obj.get("parallel_concurrency")?.as_u64()? as usize;
    if concurrency == 0 {
        return None;
    }
    Some((group.to_string(), concurrency))
}

fn task_parallel_group(task: &crate::models::Task) -> Option<String> {
    let params = task.params.as_ref().map(json_inner)?;
    let params_obj = params.as_object()?;
    params_obj
        .get("parallel_group")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn is_parallel_group_active(status: TaskStatus) -> bool {
    matches!(status, TaskStatus::Dispatched | TaskStatus::Running)
}

fn filter_tasks_by_parallel_concurrency(
    tasks: Vec<crate::models::TaskWithWorkflow>,
    run_tasks_by_run: &HashMap<Uuid, Vec<crate::models::Task>>,
) -> Vec<crate::models::TaskWithWorkflow> {
    let mut active_counts: HashMap<(Uuid, String), usize> = HashMap::new();
    for (run_id, run_tasks) in run_tasks_by_run {
        for task in run_tasks {
            if !is_parallel_group_active(task.status) {
                continue;
            }
            let Some(group) = task_parallel_group(task) else {
                continue;
            };
            *active_counts.entry((*run_id, group)).or_insert(0) += 1;
        }
    }

    let mut allowed = Vec::with_capacity(tasks.len());
    for task in tasks {
        let Some((group, concurrency)) = task_parallel_group_limit(&task) else {
            allowed.push(task);
            continue;
        };

        let key = (task.run_id, group);
        let active = active_counts.get(&key).copied().unwrap_or(0);
        if active >= concurrency {
            continue;
        }
        active_counts.insert(key, active + 1);
        allowed.push(task);
    }

    allowed
}

impl<D: Database + 'static, E: ExecutorManager + 'static> Scheduler<D, E> {
    pub(super) async fn process_pending_runs(&self) -> Result<usize> {
        let runs = self.db.get_pending_runs().await?;

        if runs.is_empty() {
            return Ok(0);
        }

        let count = runs.len();

        let workflow_ids: Vec<Uuid> = runs
            .iter()
            .map(|r| r.workflow_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let workflows = self.db.get_workflows_by_ids(&workflow_ids).await?;
        let workflow_map: HashMap<Uuid, Workflow> =
            workflows.into_iter().map(|w| (w.id, w)).collect();

        let mut workflow_tasks_map: HashMap<Uuid, Vec<WorkflowTask>> = HashMap::new();
        for workflow_id in workflow_map.keys() {
            let tasks = self.db.list_workflow_tasks(*workflow_id).await?;
            if !tasks.is_empty() {
                workflow_tasks_map.insert(*workflow_id, tasks);
            }
        }

        // Phase 1: build task lists in memory, collecting dag runs for a single batch insert.
        // Non-dag runs (batch_create_tasks) are infrequent and handled individually.
        let mut dag_tasks: Vec<(Uuid, Vec<crate::database::NewTask>)> = Vec::new();
        let mut runs_to_start: Vec<&crate::models::Run> = Vec::new();
        let mut failed_runs: Vec<(Uuid, String)> = Vec::new();

        for run in &runs {
            let Some(workflow) = workflow_map.get(&run.workflow_id) else {
                continue;
            };
            let launch_config =
                match crate::models::RunLaunchConfig::parse(run.task_selection.as_ref()) {
                    Ok(value) => value,
                    Err(e) => {
                        failed_runs.push((run.id, e.to_string()));
                        continue;
                    }
                };
            let parsed_selection = launch_config.task_selection.as_ref();
            let run_inputs = launch_config
                .inputs
                .as_ref()
                .and_then(|value| value.as_object());

            if let Some(snapshot_id) = run.snapshot_id {
                match self.get_snapshot_cached(snapshot_id).await {
                    Ok(snapshot) => {
                        let tasks_json = json_inner(&snapshot.tasks_json);
                        match serde_json::from_value::<Vec<crate::database::NewWorkflowTask>>(
                            tasks_json.clone(),
                        ) {
                            Ok(snapshot_tasks) => {
                                let tasks = crate::task_execution::build_run_tasks_from_snapshot(
                                    run.id,
                                    workflow,
                                    &snapshot_tasks,
                                    run_inputs,
                                );
                                let tasks = if let Some(selection) = parsed_selection.as_ref() {
                                    crate::task_execution::filter_run_tasks_by_selection(
                                        tasks, selection,
                                    )
                                } else {
                                    Ok(tasks)
                                };
                                match tasks {
                                    Ok(tasks) => {
                                        dag_tasks.push((run.id, tasks));
                                        runs_to_start.push(run);
                                    }
                                    Err(err) => {
                                        failed_runs.push((run.id, err.to_string()));
                                    }
                                }
                            }
                            Err(e) => {
                                failed_runs.push((
                                    run.id,
                                    format!(
                                        "Failed to deserialize snapshot tasks for run {}: {}",
                                        run.id, e
                                    ),
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        failed_runs.push((
                            run.id,
                            format!(
                                "Failed to load snapshot {} for run {}: {}",
                                snapshot_id, run.id, e
                            ),
                        ));
                    }
                }
            } else if let Some(workflow_tasks) = workflow_tasks_map.get(&run.workflow_id) {
                let tasks = build_run_tasks(run.id, workflow, workflow_tasks, run_inputs);
                let tasks = if let Some(selection) = parsed_selection.as_ref() {
                    crate::task_execution::filter_run_tasks_by_selection(tasks, selection)
                } else {
                    Ok(tasks)
                };
                match tasks {
                    Ok(tasks) => {
                        dag_tasks.push((run.id, tasks));
                        runs_to_start.push(run);
                    }
                    Err(err) => {
                        failed_runs.push((run.id, err.to_string()));
                    }
                }
            } else if workflow.executor_type == "dag" {
                failed_runs.push((
                    run.id,
                    format!(
                        "workflow '{}' is missing compiled workflow_tasks",
                        workflow.name
                    ),
                ));
            } else {
                let task_count = workflow
                    .task_params
                    .as_ref()
                    .and_then(|p| json_inner(p).get("task_count"))
                    .and_then(|v: &serde_json::Value| v.as_i64())
                    .unwrap_or(3) as i32;
                if let Err(e) = self
                    .db
                    .batch_create_tasks(run.id, task_count, &workflow.name, &workflow.executor_type)
                    .await
                {
                    failed_runs.push((run.id, e.to_string()));
                } else {
                    runs_to_start.push(run);
                }
            }
        }

        // Phase 2: fail runs whose task lists couldn't be built.
        for (run_id, err) in &failed_runs {
            error!("Failed to create tasks for run {}: {}", run_id, err);
            let _ = self
                .db
                .update_run_status(*run_id, RunStatus::Failed, Some(err))
                .await;
        }

        // Phase 3: insert all dag tasks in one round-trip.
        if !dag_tasks.is_empty()
            && let Err(e) = self.db.batch_create_dag_tasks_multi(&dag_tasks).await
        {
            error!("Failed to batch create dag tasks: {}", e);
            // Fall back: mark all dag runs as failed since we can't recover per-run here.
            for (run_id, _) in &dag_tasks {
                let _ = self
                    .db
                    .update_run_status(*run_id, RunStatus::Failed, Some(&e.to_string()))
                    .await;
            }
            runs_to_start.retain(|r| !dag_tasks.iter().any(|(id, _)| *id == r.id));
        }

        // Phase 4: start all successful runs in one round-trip.
        if !runs_to_start.is_empty() {
            let start_updates: Vec<(Uuid, RunStatus, Option<String>)> = runs_to_start
                .iter()
                .map(|run| (run.id, RunStatus::Running, None))
                .collect();
            match self.db.batch_update_run_statuses(&start_updates).await {
                Err(e) => {
                    error!("Failed to batch update runs to running: {}", e);
                }
                Ok(()) => {
                    for run in &runs_to_start {
                        self.cache_run_meta(run.id, run.workflow_id, run.snapshot_id);
                        self.fire_run_start_hooks(run.id, run.snapshot_id, &run.workflow_id)
                            .await;
                    }
                }
            }
        }

        Ok(count)
    }

    pub(super) async fn process_pending_tasks(&self) -> Result<usize> {
        let tasks = self
            .db
            .get_pending_tasks_with_workflow(self.config.max_tasks_per_batch)
            .await?;

        if tasks.is_empty() {
            return Ok(0);
        }

        let count = tasks.len();

        let workflow_ids: Vec<Uuid> = tasks
            .iter()
            .map(|task| task.workflow_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let workflows = self.db.get_workflows_by_ids(&workflow_ids).await?;
        let workflow_map: HashMap<Uuid, Workflow> =
            workflows.into_iter().map(|w| (w.id, w)).collect();
        let workflow_map = Arc::new(workflow_map);

        let mut deps_by_run: HashMap<Uuid, HashSet<String>> = HashMap::new();
        let mut deps_by_workflow: HashMap<Uuid, HashSet<String>> = HashMap::new();
        for task in &tasks {
            if task.depends_on.is_empty() {
                continue;
            }
            let deps = deps_by_run.entry(task.run_id).or_default();
            let workflow_deps = deps_by_workflow.entry(task.workflow_id).or_default();
            for name in &task.depends_on {
                deps.insert(name.clone());
                workflow_deps.insert(name.clone());
            }
        }

        let mut outputs_by_run: HashMap<Uuid, HashMap<String, serde_json::Value>> = HashMap::new();
        let mut available_run_tasks_by_run: HashMap<Uuid, HashSet<String>> = HashMap::new();
        let mut dependency_statuses_by_run: HashMap<Uuid, HashMap<String, TaskStatus>> =
            HashMap::new();
        let mut run_tasks_by_run: HashMap<Uuid, Vec<crate::models::Task>> = HashMap::new();
        for (run_id, deps) in deps_by_run {
            match self.db.list_tasks(run_id).await {
                Ok(run_tasks) => {
                    run_tasks_by_run.insert(run_id, run_tasks.clone());
                    let mut statuses = HashMap::new();
                    let names = run_tasks
                        .into_iter()
                        .map(|task| {
                            statuses.insert(task.task_name.clone(), task.status);
                            task.task_name
                        })
                        .collect::<HashSet<_>>();
                    available_run_tasks_by_run.insert(run_id, names);
                    dependency_statuses_by_run.insert(run_id, statuses);
                }
                Err(err) => {
                    error!("Failed to load run task names for run {}: {}", run_id, err);
                }
            }

            let names: Vec<String> = deps.into_iter().collect();
            match self.db.get_task_outputs(run_id, &names).await {
                Ok(map) => {
                    outputs_by_run.insert(run_id, map);
                }
                Err(err) => {
                    error!(
                        "Failed to load upstream outputs for run {}: {}",
                        run_id, err
                    );
                }
            }
        }

        let concurrency_run_ids: Vec<Uuid> = tasks
            .iter()
            .filter_map(|task| task_parallel_group_limit(task).map(|_| task.run_id))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        for run_id in concurrency_run_ids {
            if run_tasks_by_run.contains_key(&run_id) {
                continue;
            }
            match self.db.list_tasks(run_id).await {
                Ok(run_tasks) => {
                    run_tasks_by_run.insert(run_id, run_tasks);
                }
                Err(err) => {
                    error!(
                        "Failed to load run tasks for concurrency gating on run {}: {}",
                        run_id, err
                    );
                }
            }
        }

        let mut persisted_outputs_by_workflow: HashMap<Uuid, HashMap<String, serde_json::Value>> =
            HashMap::new();
        for (workflow_id, deps) in deps_by_workflow {
            match self.db.list_workflow_task_states(workflow_id).await {
                Ok(states) => {
                    let mut outputs = HashMap::new();
                    for state in states {
                        if !deps.contains(&state.task_name)
                            || !matches!(state.last_status, TaskStatus::Success)
                        {
                            continue;
                        }
                        if let Some(output) = state.last_output {
                            outputs.insert(state.task_name, json_inner(&output).clone());
                        }
                    }
                    persisted_outputs_by_workflow.insert(workflow_id, outputs);
                }
                Err(err) => {
                    error!(
                        "Failed to load persisted task states for workflow {}: {}",
                        workflow_id, err
                    );
                }
            }
        }
        let outputs_by_run = Arc::new(outputs_by_run);
        let available_run_tasks_by_run = Arc::new(available_run_tasks_by_run);
        let dependency_statuses_by_run = Arc::new(dependency_statuses_by_run);
        let persisted_outputs_by_workflow = Arc::new(persisted_outputs_by_workflow);

        let mut materialized_dynamic_fanout = false;
        for task in &tasks {
            if self
                .materialize_dynamic_fanout(
                    task,
                    outputs_by_run.as_ref(),
                    available_run_tasks_by_run.as_ref(),
                    persisted_outputs_by_workflow.as_ref(),
                )
                .await?
            {
                materialized_dynamic_fanout = true;
            }
        }
        if materialized_dynamic_fanout {
            info!("Materialized runtime fan-out; deferring dispatch until next scheduler pass");
            return Ok(count);
        }

        let tasks = filter_tasks_by_parallel_concurrency(tasks, &run_tasks_by_run);

        // Warm the task identity cache so status_processing doesn't need DB round-trips.
        for task in &tasks {
            self.cache_task_identity(task.task_id, task.run_id, task.task_name.clone());
        }

        let results = stream::iter(tasks)
            .map(|task_with_workflow| {
                let executor_manager = Arc::clone(&self.executor_manager);
                let status_tx = self.status_tx.clone();
                let workflow_map = workflow_map.clone();
                let outputs_by_run = outputs_by_run.clone();
                let available_run_tasks_by_run = available_run_tasks_by_run.clone();
                let dependency_statuses_by_run = dependency_statuses_by_run.clone();
                let persisted_outputs_by_workflow = persisted_outputs_by_workflow.clone();

                async move {
                    execute_task(
                        task_with_workflow,
                        workflow_map,
                        outputs_by_run,
                        available_run_tasks_by_run,
                        dependency_statuses_by_run,
                        persisted_outputs_by_workflow,
                        &*executor_manager,
                        status_tx,
                    )
                    .await
                }
            })
            .buffer_unordered(self.config.max_concurrent_dispatches)
            .collect::<Vec<_>>()
            .await;

        let mut dispatch_updates: Vec<(Uuid, String)> = Vec::new();
        let mut skip_updates: Vec<StatusUpdate> = Vec::new();
        let mut failure_updates: Vec<StatusUpdate> = Vec::new();

        for (task_id, result) in results {
            match result {
                Ok(TaskOutcome::Dispatched(execution_name)) => {
                    dispatch_updates.push((task_id, execution_name));
                }
                Ok(TaskOutcome::Skipped) => {
                    skip_updates.push(StatusUpdate {
                        task_id,
                        status: "skipped".to_string(),
                        log: None,
                        output: None,
                        error: None,
                        progress: None,
                    });
                }
                Err(e) => {
                    error!("Failed to dispatch task {}: {}", task_id, e);
                    failure_updates.push(StatusUpdate {
                        task_id,
                        status: "failed".to_string(),
                        log: None,
                        output: None,
                        error: Some(e.to_string()),
                        progress: None,
                    });
                }
            }
        }

        let db_update_start = Instant::now();
        if !dispatch_updates.is_empty() {
            let updates_ref: Vec<(Uuid, TaskStatus, Option<&str>, Option<&str>)> = dispatch_updates
                .iter()
                .map(|(id, execution)| {
                    (*id, TaskStatus::Dispatched, Some(execution.as_str()), None)
                })
                .collect();
            self.db.batch_update_task_status(&updates_ref).await?;
        }
        let db_update_ms = db_update_start.elapsed().as_millis();
        info!(
            "Batch updated {} tasks in {}ms",
            dispatch_updates.len(),
            db_update_ms
        );

        if !skip_updates.is_empty() {
            self.process_status_updates(skip_updates).await?;
        }

        if !failure_updates.is_empty() {
            self.process_status_updates(failure_updates).await?;
        }

        Ok(count)
    }

    pub(super) async fn process_scheduled_triggers(&self) -> Result<usize> {
        crate::schedule_processor::process_scheduled_triggers(self.db.as_ref()).await
    }
}
