/// Hook dispatch logic — fires lifecycle hooks when task/run events occur.
///
/// Hooks are stored in the workflow snapshot alongside regular tasks but with
/// `executor_type = "hook"`. They are skipped during run task creation and
/// dispatched here when the matching event fires.
///
/// Hook executors receive a `task_input` with event context. The hook runs as
/// a fire-and-forget background task — its outcome does not affect the run.
use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{error, info};
use uuid::Uuid;

use crate::database::NewWorkflowTask;
use crate::executor::StatusUpdate;
use crate::executor_manager::ExecutorManager;
use crate::models::Workflow;
use crate::task_execution::HOOK_EXECUTOR_TYPE;

/// Context passed to a hook executor as `task_input`.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct HookContext {
    /// The event name (e.g. `"task.failed"`).
    pub event: String,
    /// The hook definition name.
    pub hook_name: String,
    /// UUID of the run that triggered the event.
    pub run_id: String,
    /// Name of the workflow.
    pub workflow_name: String,
    /// For task-level events: the task that triggered the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_name: Option<String>,
    /// For task.failed / run.failed: error message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// For task.success: task output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    /// For run.failed: list of failed task names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_tasks: Option<Vec<String>>,
}

/// Dispatch all hooks that match the given task-level event.
///
/// `snapshot_tasks` should be the full snapshot task list (including hook entries).
/// Each matching hook is fired in a background tokio task (fire-and-forget).
pub async fn fire_task_hooks<E: ExecutorManager>(
    event: &str,
    run_id: Uuid,
    task_name: &str,
    error: Option<&str>,
    output: Option<serde_json::Value>,
    snapshot_tasks: &[NewWorkflowTask],
    workflow: &Workflow,
    executor_manager: &Arc<E>,
    status_tx: mpsc::UnboundedSender<StatusUpdate>,
) {
    for hook_task in snapshot_tasks {
        if hook_task.executor_type != HOOK_EXECUTOR_TYPE {
            continue;
        }
        let params = &hook_task.params;
        let hook_event = params
            .get("hook_event")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if hook_event != event {
            continue;
        }

        // Check tasks filter
        let tasks_filter = params.get("hook_tasks_filter");
        let matches = match tasks_filter {
            None | Some(serde_json::Value::Null) => true,
            Some(serde_json::Value::Array(names)) => {
                names.iter().any(|n| n.as_str() == Some(task_name))
            }
            _ => true, // unknown filter format — fire for all
        };
        if !matches {
            continue;
        }

        let hook_name = hook_task
            .task_name
            .strip_prefix("__hook__:")
            .unwrap_or(&hook_task.task_name)
            .to_string();

        let context = HookContext {
            event: event.to_string(),
            hook_name: hook_name.clone(),
            run_id: run_id.to_string(),
            workflow_name: workflow.name.clone(),
            task_name: Some(task_name.to_string()),
            error: error.map(|e| e.to_string()),
            output: output.clone(),
            failed_tasks: None,
        };

        dispatch_hook(
            hook_name,
            context,
            hook_task,
            workflow,
            executor_manager,
            status_tx.clone(),
        )
        .await;
    }
}

/// Dispatch all hooks that match the given run-level event.
pub async fn fire_run_hooks<E: ExecutorManager>(
    event: &str,
    run_id: Uuid,
    error: Option<&str>,
    failed_tasks: Option<Vec<String>>,
    snapshot_tasks: &[NewWorkflowTask],
    workflow: &Workflow,
    executor_manager: &Arc<E>,
    status_tx: mpsc::UnboundedSender<StatusUpdate>,
) {
    for hook_task in snapshot_tasks {
        if hook_task.executor_type != HOOK_EXECUTOR_TYPE {
            continue;
        }
        let params = &hook_task.params;
        let hook_event = params
            .get("hook_event")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if hook_event != event {
            continue;
        }

        let hook_name = hook_task
            .task_name
            .strip_prefix("__hook__:")
            .unwrap_or(&hook_task.task_name)
            .to_string();

        let context = HookContext {
            event: event.to_string(),
            hook_name: hook_name.clone(),
            run_id: run_id.to_string(),
            workflow_name: workflow.name.clone(),
            task_name: None,
            error: error.map(|e| e.to_string()),
            output: None,
            failed_tasks: failed_tasks.clone(),
        };

        dispatch_hook(
            hook_name,
            context,
            hook_task,
            workflow,
            executor_manager,
            status_tx.clone(),
        )
        .await;
    }
}

async fn dispatch_hook<E: ExecutorManager>(
    hook_name: String,
    context: HookContext,
    hook_task: &NewWorkflowTask,
    workflow: &Workflow,
    executor_manager: &Arc<E>,
    status_tx: mpsc::UnboundedSender<StatusUpdate>,
) {
    let params = &hook_task.params;
    let executor_type = params
        .get("hook_executor_type")
        .and_then(|v| v.as_str())
        .unwrap_or("python");

    let executor = match executor_manager.get_executor(executor_type, workflow).await {
        Ok(e) => e,
        Err(err) => {
            error!(
                "Hook '{}': failed to get executor '{}': {}",
                hook_name, executor_type, err
            );
            return;
        }
    };

    executor.set_status_channel(status_tx).await;

    // Build params for the hook executor — same shape as a regular task.
    let mut exec_params = params.clone();
    if let Some(obj) = exec_params.as_object_mut() {
        // Pass context as task_input so the hook function receives it.
        if let Ok(ctx_value) = serde_json::to_value(&context) {
            obj.insert("task_input".to_string(), ctx_value);
        }
        // Remove hook-internal keys that executors don't need
        obj.remove("hook_event");
        obj.remove("hook_executor_type");
        obj.remove("hook_tasks_filter");
    }

    // Generate a unique ID for this hook invocation (not stored in DB).
    let hook_task_id = Uuid::new_v4();
    let job_name = params
        .get("job_name")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("command").and_then(|v| v.as_str()))
        .unwrap_or(&workflow.job_name)
        .to_string();

    let executor_clone = Arc::clone(&executor);
    let hook_name_clone = hook_name.clone();

    tokio::spawn(async move {
        match executor_clone
            .execute(hook_task_id, &job_name, Some(exec_params))
            .await
        {
            Ok(_) => info!(
                "Hook '{}' dispatched (id={})",
                hook_name_clone, hook_task_id
            ),
            Err(err) => error!("Hook '{}' failed to dispatch: {}", hook_name_clone, err),
        }
    });
}
