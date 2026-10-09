use super::*;
use crate::hooks;

impl<D: Database + 'static, E: ExecutorManager + 'static> Scheduler<D, E> {
    /// Load the snapshot task list for a given run, returning hook entries.
    async fn load_hook_tasks(
        &self,
        snapshot_id: Option<uuid::Uuid>,
    ) -> Vec<crate::database::NewWorkflowTask> {
        let Some(sid) = snapshot_id else {
            return vec![];
        };
        match self.get_snapshot_cached(sid).await {
            Ok(snapshot) => {
                let tasks_json = crate::models::json_inner(&snapshot.tasks_json);
                match serde_json::from_value::<Vec<crate::database::NewWorkflowTask>>(
                    tasks_json.clone(),
                ) {
                    Ok(tasks) => tasks
                        .into_iter()
                        .filter(|t| t.executor_type == crate::task_execution::HOOK_EXECUTOR_TYPE)
                        .collect(),
                    Err(e) => {
                        error!(
                            "Failed to deserialize snapshot {} tasks for hooks: {}",
                            sid, e
                        );
                        vec![]
                    }
                }
            }
            Err(e) => {
                error!("Failed to load snapshot {} for hook dispatch: {}", sid, e);
                vec![]
            }
        }
    }

    pub(super) async fn fire_task_hooks(
        &self,
        event: &str,
        run_id: Uuid,
        snapshot_id: Option<Uuid>,
        workflow_id: &Uuid,
        task_name: &str,
        error: Option<&str>,
        output: Option<serde_json::Value>,
    ) {
        let hook_tasks = self.load_hook_tasks(snapshot_id).await;
        if hook_tasks.is_empty() {
            return;
        }
        let workflow = match self.db.get_workflow_by_id(*workflow_id).await {
            Ok(w) => w,
            Err(e) => {
                error!(
                    "Failed to load workflow {} for hook dispatch: {}",
                    workflow_id, e
                );
                return;
            }
        };
        hooks::fire_task_hooks(
            event,
            run_id,
            task_name,
            error,
            output,
            &hook_tasks,
            &workflow,
            &self.executor_manager,
            self.status_tx.clone(),
        )
        .await;
    }

    pub(super) async fn fire_run_hooks(
        &self,
        event: &str,
        run_id: Uuid,
        snapshot_id: Option<Uuid>,
        workflow_id: &Uuid,
        error: Option<&str>,
        failed_tasks: Option<Vec<String>>,
    ) {
        let hook_tasks = self.load_hook_tasks(snapshot_id).await;
        if hook_tasks.is_empty() {
            return;
        }
        let workflow = match self.db.get_workflow_by_id(*workflow_id).await {
            Ok(w) => w,
            Err(e) => {
                error!(
                    "Failed to load workflow {} for hook dispatch: {}",
                    workflow_id, e
                );
                return;
            }
        };
        hooks::fire_run_hooks(
            event,
            run_id,
            error,
            failed_tasks,
            &hook_tasks,
            &workflow,
            &self.executor_manager,
            self.status_tx.clone(),
        )
        .await;
    }

    pub(super) async fn fire_run_start_hooks(
        &self,
        run_id: Uuid,
        snapshot_id: Option<Uuid>,
        workflow_id: &Uuid,
    ) {
        self.fire_run_hooks("run.start", run_id, snapshot_id, workflow_id, None, None)
            .await;
    }
}
