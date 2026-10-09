/// Instrumented Database wrapper for profiling DB call counts and timing.
///
/// Activated by `CZAR_DB_TRACE=1`. Zero overhead when not enabled — the wrapper
/// is never instantiated in that case. When active, each DB method records its
/// call count and total elapsed time, emitting a `DB_TRACE: {json}` line to
/// stderr every 2 seconds for parsing by perf-test.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::database::{
    Database, DeferredJobRepository, NewTask, NewWorkflowTask, RunListPage, RunListQuery,
    RunRepository, ScheduleRepository, TaskRepository, TaskRetryMeta, WorkflowListPage,
    WorkflowListQuery, WorkflowRepository, WorkflowSnapshotRepository,
};
use crate::models::{
    DeferredJob, DeferredJobStatus, Run, RunStatus, Task, TaskStatus, TaskWithWorkflow, Workflow,
    WorkflowTaskState,
};

// ── per-method stats ─────────────────────────────────────────────────────────

#[derive(Default, Clone, Serialize)]
pub struct CallStats {
    pub calls: u64,
    pub total_ms: f64,
}

// ── tracer ───────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct DbTracer {
    stats: Mutex<HashMap<&'static str, CallStats>>,
}

pub struct TraceGuard<'a> {
    tracer: &'a DbTracer,
    method: &'static str,
    start: Instant,
}

impl Drop for TraceGuard<'_> {
    fn drop(&mut self) {
        let elapsed_ms = self.start.elapsed().as_secs_f64() * 1000.0;
        let mut stats = self.tracer.stats.lock().unwrap();
        let e = stats.entry(self.method).or_default();
        e.calls += 1;
        e.total_ms += elapsed_ms;
    }
}

impl DbTracer {
    pub fn track(&self, method: &'static str) -> TraceGuard<'_> {
        TraceGuard {
            tracer: self,
            method,
            start: Instant::now(),
        }
    }

    /// Return a snapshot of current cumulative stats (does not reset counters).
    pub fn snapshot(&self) -> HashMap<&'static str, CallStats> {
        self.stats.lock().unwrap().clone()
    }
}

// ── wrapper ───────────────────────────────────────────────────────────────────

pub struct TracingDatabase<D> {
    pub(crate) inner: Arc<D>,
    pub(crate) tracer: Arc<DbTracer>,
}

impl<D: Database + 'static> TracingDatabase<D> {
    /// Wrap `inner` with call tracing. Spawns a background task that emits
    /// `DB_TRACE: {json}` to stderr every 2 seconds. Must be called from
    /// within a Tokio runtime (i.e. from an async context).
    pub fn new(inner: Arc<D>) -> Self {
        let tracer = Arc::new(DbTracer::default());
        let weak = Arc::downgrade(&tracer);

        tokio::spawn(async move {
            // First tick fires immediately — skip it so we don't emit an empty snapshot.
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            interval.tick().await;
            loop {
                interval.tick().await;
                let Some(t) = weak.upgrade() else { break };
                let snapshot = t.snapshot();
                if !snapshot.is_empty()
                    && let Ok(json) = serde_json::to_string(&snapshot)
                {
                    eprintln!("DB_TRACE: {json}");
                }
            }
        });

        Self { inner, tracer }
    }
}

// ── helper macro ─────────────────────────────────────────────────────────────

/// Wrap a single DB call: record start time, delegate, record elapsed on drop.
macro_rules! traced {
    ($self:ident . $method:ident ( $($arg:expr),* $(,)? )) => {{
        let _guard = $self.tracer.track(stringify!($method));
        $self.inner.$method($($arg),*).await
    }};
}

// ── WorkflowRepository ───────────────────────────────────────────────────────

#[async_trait]
impl<D: Database + 'static> WorkflowRepository for TracingDatabase<D> {
    async fn create_workflow(
        &self,
        name: &str,
        description: Option<&str>,
        job_name: &str,
        region: &str,
        project: &str,
        executor_type: &str,
        task_params: Option<serde_json::Value>,
        schedule: Option<&str>,
    ) -> anyhow::Result<Workflow> {
        let _guard = self.tracer.track("create_workflow");
        self.inner
            .create_workflow(
                name,
                description,
                job_name,
                region,
                project,
                executor_type,
                task_params,
                schedule,
            )
            .await
    }

    async fn get_workflow(&self, name: &str) -> anyhow::Result<Workflow> {
        traced!(self.get_workflow(name))
    }

    async fn get_workflow_by_id(&self, workflow_id: Uuid) -> anyhow::Result<Workflow> {
        traced!(self.get_workflow_by_id(workflow_id))
    }

    async fn list_workflows(&self) -> anyhow::Result<Vec<Workflow>> {
        traced!(self.list_workflows())
    }

    async fn list_workflows_page(
        &self,
        query: &WorkflowListQuery,
    ) -> anyhow::Result<WorkflowListPage> {
        traced!(self.list_workflows_page(query))
    }

    async fn delete_workflow(&self, name: &str) -> anyhow::Result<()> {
        traced!(self.delete_workflow(name))
    }

    async fn get_workflows_by_ids(&self, workflow_ids: &[Uuid]) -> anyhow::Result<Vec<Workflow>> {
        traced!(self.get_workflows_by_ids(workflow_ids))
    }

    async fn create_workflow_tasks(
        &self,
        workflow_id: Uuid,
        tasks: &[NewWorkflowTask],
    ) -> anyhow::Result<()> {
        traced!(self.create_workflow_tasks(workflow_id, tasks))
    }

    async fn update_workflow_task_params(
        &self,
        workflow_id: Uuid,
        task_params: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        traced!(self.update_workflow_task_params(workflow_id, task_params))
    }

    async fn list_workflow_tasks(
        &self,
        workflow_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::WorkflowTask>> {
        traced!(self.list_workflow_tasks(workflow_id))
    }
}

// ── WorkflowSnapshotRepository ───────────────────────────────────────────────

#[async_trait]
impl<D: Database + 'static> WorkflowSnapshotRepository for TracingDatabase<D> {
    async fn create_or_get_snapshot(
        &self,
        workflow_id: Uuid,
        content_hash: &str,
        tasks_json: serde_json::Value,
        source_yaml: Option<String>,
    ) -> anyhow::Result<crate::models::WorkflowSnapshot> {
        traced!(self.create_or_get_snapshot(workflow_id, content_hash, tasks_json, source_yaml))
    }

    async fn get_snapshot(
        &self,
        snapshot_id: Uuid,
    ) -> anyhow::Result<crate::models::WorkflowSnapshot> {
        traced!(self.get_snapshot(snapshot_id))
    }

    async fn update_workflow_snapshot(
        &self,
        workflow_id: Uuid,
        snapshot_id: Uuid,
    ) -> anyhow::Result<()> {
        traced!(self.update_workflow_snapshot(workflow_id, snapshot_id))
    }
}

// ── RunRepository ─────────────────────────────────────────────────────────────

#[async_trait]
impl<D: Database + 'static> RunRepository for TracingDatabase<D> {
    async fn create_run(&self, workflow_id: Uuid, triggered_by: &str) -> anyhow::Result<Run> {
        traced!(self.create_run(workflow_id, triggered_by))
    }

    async fn create_run_with_snapshot_and_selection(
        &self,
        workflow_id: Uuid,
        snapshot_id: Option<Uuid>,
        triggered_by: &str,
        task_selection: Option<serde_json::Value>,
    ) -> anyhow::Result<Run> {
        traced!(self.create_run_with_snapshot_and_selection(
            workflow_id,
            snapshot_id,
            triggered_by,
            task_selection,
        ))
    }

    async fn create_run_with_selection(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
        task_selection: Option<serde_json::Value>,
    ) -> anyhow::Result<Run> {
        traced!(self.create_run_with_selection(workflow_id, triggered_by, task_selection))
    }

    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: RunStatus,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        traced!(self.update_run_status(run_id, status, error))
    }

    async fn get_run(&self, run_id: Uuid) -> anyhow::Result<Run> {
        traced!(self.get_run(run_id))
    }

    async fn list_runs(&self, workflow_id: Option<Uuid>) -> anyhow::Result<Vec<Run>> {
        traced!(self.list_runs(workflow_id))
    }

    async fn list_runs_page(&self, query: &RunListQuery) -> anyhow::Result<RunListPage> {
        traced!(self.list_runs_page(query))
    }

    async fn get_pending_runs(&self) -> anyhow::Result<Vec<Run>> {
        traced!(self.get_pending_runs())
    }

    async fn cancel_run(&self, run_id: Uuid) -> anyhow::Result<()> {
        traced!(self.cancel_run(run_id))
    }

    async fn get_run_task_stats(&self, run_id: Uuid) -> anyhow::Result<(i64, i64, i64)> {
        traced!(self.get_run_task_stats(run_id))
    }

    async fn batch_get_run_task_stats(
        &self,
        run_ids: &[Uuid],
    ) -> anyhow::Result<std::collections::HashMap<Uuid, (i64, i64, i64)>> {
        traced!(self.batch_get_run_task_stats(run_ids))
    }

    async fn batch_update_run_statuses(
        &self,
        updates: &[(Uuid, RunStatus, Option<String>)],
    ) -> anyhow::Result<()> {
        traced!(self.batch_update_run_statuses(updates))
    }
}

// ── TaskRepository ────────────────────────────────────────────────────────────

#[async_trait]
impl<D: Database + 'static> TaskRepository for TracingDatabase<D> {
    async fn batch_create_tasks(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> anyhow::Result<()> {
        traced!(self.batch_create_tasks(run_id, task_count, workflow_name, executor_type))
    }

    async fn batch_create_dag_tasks(&self, run_id: Uuid, tasks: &[NewTask]) -> anyhow::Result<()> {
        traced!(self.batch_create_dag_tasks(run_id, tasks))
    }

    async fn batch_create_dag_tasks_multi(
        &self,
        runs: &[(Uuid, Vec<NewTask>)],
    ) -> anyhow::Result<()> {
        traced!(self.batch_create_dag_tasks_multi(runs))
    }

    async fn replace_pending_task(
        &self,
        task_id: Uuid,
        original_task_name: &str,
        replacement_tasks: &[NewTask],
    ) -> anyhow::Result<()> {
        traced!(self.replace_pending_task(task_id, original_task_name, replacement_tasks))
    }

    async fn update_task_status(
        &self,
        task_id: Uuid,
        status: TaskStatus,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        traced!(self.update_task_status(task_id, status, execution_name, error))
    }

    async fn batch_update_task_status(
        &self,
        updates: &[(Uuid, TaskStatus, Option<&str>, Option<&str>)],
    ) -> anyhow::Result<()> {
        traced!(self.batch_update_task_status(updates))
    }

    async fn list_tasks(&self, run_id: Uuid) -> anyhow::Result<Vec<Task>> {
        traced!(self.list_tasks(run_id))
    }

    async fn get_pending_tasks(&self) -> anyhow::Result<Vec<Task>> {
        traced!(self.get_pending_tasks())
    }

    async fn get_running_tasks(&self) -> anyhow::Result<Vec<Task>> {
        traced!(self.get_running_tasks())
    }

    async fn batch_append_task_logs(&self, entries: &[(Uuid, String)]) -> anyhow::Result<()> {
        traced!(self.batch_append_task_logs(entries))
    }

    async fn update_task_output(
        &self,
        task_id: Uuid,
        output: serde_json::Value,
    ) -> anyhow::Result<()> {
        traced!(self.update_task_output(task_id, output))
    }

    async fn update_task_progress(
        &self,
        task_id: Uuid,
        progress: serde_json::Value,
    ) -> anyhow::Result<()> {
        traced!(self.update_task_progress(task_id, progress))
    }

    async fn reset_task_for_retry(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        traced!(self.reset_task_for_retry(task_id, error, retry_at))
    }

    async fn get_pending_tasks_with_workflow(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<TaskWithWorkflow>> {
        traced!(self.get_pending_tasks_with_workflow(limit))
    }

    async fn get_task_outputs(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> anyhow::Result<HashMap<String, serde_json::Value>> {
        traced!(self.get_task_outputs(run_id, task_names))
    }

    async fn get_task_retry_meta(
        &self,
        task_ids: &[Uuid],
    ) -> anyhow::Result<HashMap<Uuid, TaskRetryMeta>> {
        traced!(self.get_task_retry_meta(task_ids))
    }

    async fn get_task_run_id(&self, task_id: Uuid) -> anyhow::Result<Uuid> {
        traced!(self.get_task_run_id(task_id))
    }

    async fn get_task_identity(&self, task_id: Uuid) -> anyhow::Result<(Uuid, String)> {
        traced!(self.get_task_identity(task_id))
    }

    async fn mark_tasks_failed_by_dependency(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> anyhow::Result<Vec<String>> {
        traced!(self.mark_tasks_failed_by_dependency(run_id, failed_task_names, error))
    }

    async fn upsert_workflow_task_state(
        &self,
        workflow_id: Uuid,
        task_name: &str,
        last_status: TaskStatus,
        last_run_id: Uuid,
        last_task_id: Uuid,
        last_output: Option<serde_json::Value>,
        last_error: Option<&str>,
        last_finished_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        let _guard = self.tracer.track("upsert_workflow_task_state");
        self.inner
            .upsert_workflow_task_state(
                workflow_id,
                task_name,
                last_status,
                last_run_id,
                last_task_id,
                last_output,
                last_error,
                last_finished_at,
            )
            .await
    }

    async fn batch_upsert_workflow_task_states(
        &self,
        entries: &[crate::database::WorkflowTaskStateEntry],
    ) -> anyhow::Result<()> {
        traced!(self.batch_upsert_workflow_task_states(entries))
    }

    async fn list_workflow_task_states(
        &self,
        workflow_id: Uuid,
    ) -> anyhow::Result<Vec<WorkflowTaskState>> {
        traced!(self.list_workflow_task_states(workflow_id))
    }
}

// ── ScheduleRepository ───────────────────────────────────────────────────────

#[async_trait]
impl<D: Database + 'static> ScheduleRepository for TracingDatabase<D> {
    async fn get_due_scheduled_workflows(&self) -> anyhow::Result<Vec<Workflow>> {
        traced!(self.get_due_scheduled_workflows())
    }

    async fn update_workflow_schedule_times(
        &self,
        workflow_id: Uuid,
        last_scheduled_at: DateTime<Utc>,
        next_scheduled_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        traced!(self.update_workflow_schedule_times(
            workflow_id,
            last_scheduled_at,
            next_scheduled_at,
        ))
    }

    async fn update_workflow_schedule(
        &self,
        workflow_id: Uuid,
        schedule: Option<&str>,
        schedule_enabled: bool,
    ) -> anyhow::Result<()> {
        traced!(self.update_workflow_schedule(workflow_id, schedule, schedule_enabled))
    }
}

// ── DeferredJobRepository ────────────────────────────────────────────────────

#[async_trait]
impl<D: Database + 'static> DeferredJobRepository for TracingDatabase<D> {
    async fn create_deferred_job(
        &self,
        task_id: Uuid,
        service_type: &str,
        job_id: &str,
        job_data: serde_json::Value,
    ) -> anyhow::Result<DeferredJob> {
        traced!(self.create_deferred_job(task_id, service_type, job_id, job_data))
    }

    async fn get_pending_deferred_jobs(&self) -> anyhow::Result<Vec<DeferredJob>> {
        traced!(self.get_pending_deferred_jobs())
    }

    async fn get_deferred_jobs_for_task(&self, task_id: Uuid) -> anyhow::Result<Vec<DeferredJob>> {
        traced!(self.get_deferred_jobs_for_task(task_id))
    }

    async fn update_deferred_job_status(
        &self,
        job_id: Uuid,
        status: DeferredJobStatus,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        traced!(self.update_deferred_job_status(job_id, status, error))
    }

    async fn update_deferred_job_polled(&self, job_id: Uuid) -> anyhow::Result<()> {
        traced!(self.update_deferred_job_polled(job_id))
    }

    async fn complete_deferred_job(&self, job_id: Uuid) -> anyhow::Result<()> {
        traced!(self.complete_deferred_job(job_id))
    }

    async fn fail_deferred_job(&self, job_id: Uuid, error: &str) -> anyhow::Result<()> {
        traced!(self.fail_deferred_job(job_id, error))
    }

    async fn cancel_deferred_jobs_for_task(&self, task_id: Uuid) -> anyhow::Result<()> {
        traced!(self.cancel_deferred_jobs_for_task(task_id))
    }
}

// ── Database (top-level) ─────────────────────────────────────────────────────

#[async_trait]
impl<D: Database + 'static> Database for TracingDatabase<D> {
    async fn run_migrations(&self) -> anyhow::Result<()> {
        traced!(self.run_migrations())
    }
}
