use anyhow::{Result, anyhow};

use crate::database::NewTask;
use crate::models::json_inner;
use crate::workflow::ParallelMode;

use super::*;

impl<D: Database + 'static, E: ExecutorManager + 'static> Scheduler<D, E> {
    pub(super) async fn materialize_dynamic_fanout(
        &self,
        task: &crate::models::TaskWithWorkflow,
        outputs_by_run: &HashMap<Uuid, HashMap<String, serde_json::Value>>,
        available_run_tasks_by_run: &HashMap<Uuid, HashSet<String>>,
        persisted_outputs_by_workflow: &HashMap<Uuid, HashMap<String, serde_json::Value>>,
    ) -> Result<bool> {
        let Some(params) = task.params.as_ref().map(json_inner) else {
            return Ok(false);
        };
        let Some(params_obj) = params.as_object() else {
            return Ok(false);
        };

        let parallel_ref = params_obj.get("parallel_ref").and_then(|v| v.as_str());
        let foreach_ref = params_obj.get("foreach_ref").and_then(|v| v.as_str());
        if parallel_ref.is_none() && foreach_ref.is_none() {
            return Ok(false);
        }

        let mode = match params_obj
            .get("parallel_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("czar")
        {
            "czar" => ParallelMode::Czar,
            "cloud_run_tasks" => ParallelMode::CloudRunTasks,
            other => {
                return Err(anyhow!(
                    "task '{}' has unsupported parallel_mode '{}'",
                    task.task_name,
                    other
                ));
            }
        };

        let run_outputs = outputs_by_run.get(&task.run_id);
        let run_tasks = available_run_tasks_by_run.get(&task.run_id);
        let workflow_outputs = persisted_outputs_by_workflow.get(&task.workflow_id);

        let replacement_tasks = if let Some(reference) = parallel_ref {
            let count_value = crate::task_execution::resolve_upstream_output_selector(
                task,
                reference,
                run_outputs,
                run_tasks,
                workflow_outputs,
            )?;
            let count = count_value.as_i64().ok_or_else(|| {
                anyhow!(
                    "task '{}' dynamic parallel '{}' resolved to non-integer value {}",
                    task.task_name,
                    reference,
                    count_value
                )
            })?;
            if count <= 0 {
                return Err(anyhow!(
                    "task '{}' dynamic parallel '{}' resolved to {}; expected at least 1",
                    task.task_name,
                    reference,
                    count
                ));
            }
            self.build_parallel_replacements(task, params_obj, mode, count as u32)
                .await?
        } else {
            let reference = foreach_ref.expect("checked above");
            let items_value = crate::task_execution::resolve_upstream_output_selector(
                task,
                reference,
                run_outputs,
                run_tasks,
                workflow_outputs,
            )?;
            let items = items_value.as_array().ok_or_else(|| {
                anyhow!(
                    "task '{}' foreach '{}' resolved to non-array value {}",
                    task.task_name,
                    reference,
                    items_value
                )
            })?;
            if items.is_empty() {
                return Err(anyhow!(
                    "task '{}' foreach '{}' resolved to an empty array; expected at least one item",
                    task.task_name,
                    reference
                ));
            }
            if mode == ParallelMode::CloudRunTasks {
                return Err(anyhow!(
                    "task '{}' uses foreach with parallel_mode cloud_run_tasks, which cannot pass per-item inputs",
                    task.task_name
                ));
            }
            self.build_foreach_replacements(task, params_obj, items)
                .await?
        };

        self.db
            .replace_pending_task(task.task_id, &task.task_name, &replacement_tasks)
            .await?;
        Ok(true)
    }

    async fn build_parallel_replacements(
        &self,
        task: &crate::models::TaskWithWorkflow,
        params_obj: &serde_json::Map<String, serde_json::Value>,
        mode: ParallelMode,
        count: u32,
    ) -> Result<Vec<NewTask>> {
        let task_indices = self
            .db
            .list_tasks(task.run_id)
            .await?
            .into_iter()
            .map(|task| task.task_index)
            .collect::<Vec<_>>();
        let max_index = task_indices.into_iter().max().unwrap_or(task.task_index);

        let mut base_params = params_obj.clone();
        base_params.remove("parallel_ref");
        base_params.remove("foreach_ref");
        base_params.remove("task_count");
        base_params.insert(
            "parallel_group".to_string(),
            serde_json::Value::String(task.task_name.clone()),
        );

        if mode == ParallelMode::CloudRunTasks {
            base_params.insert("task_count".to_string(), serde_json::json!(count));
            base_params.insert("parallel_count".to_string(), serde_json::json!(count));
            return Ok(vec![NewTask {
                task_index: task.task_index,
                task_name: task.task_name.clone(),
                executor_type: task.executor_type.clone(),
                depends_on: task.depends_on.clone(),
                params: serde_json::Value::Object(base_params),
                max_retries: task.max_retries,
                timeout_seconds: task.timeout_seconds,
            }]);
        }

        let mut replacements = Vec::with_capacity(count as usize);
        for index in 0..count as usize {
            let mut params = base_params.clone();
            params.insert("parallel_index".to_string(), serde_json::json!(index));
            params.insert("parallel_count".to_string(), serde_json::json!(count));
            replacements.push(NewTask {
                task_index: if index == 0 {
                    task.task_index
                } else {
                    max_index + index as i32
                },
                task_name: format!("{}[{}]", task.task_name, index),
                executor_type: task.executor_type.clone(),
                depends_on: task.depends_on.clone(),
                params: serde_json::Value::Object(params),
                max_retries: task.max_retries,
                timeout_seconds: task.timeout_seconds,
            });
        }
        Ok(replacements)
    }

    async fn build_foreach_replacements(
        &self,
        task: &crate::models::TaskWithWorkflow,
        params_obj: &serde_json::Map<String, serde_json::Value>,
        items: &[serde_json::Value],
    ) -> Result<Vec<NewTask>> {
        let task_indices = self
            .db
            .list_tasks(task.run_id)
            .await?
            .into_iter()
            .map(|task| task.task_index)
            .collect::<Vec<_>>();
        let max_index = task_indices.into_iter().max().unwrap_or(task.task_index);
        let count = items.len();

        let mut base_params = params_obj.clone();
        base_params.remove("parallel_ref");
        base_params.remove("foreach_ref");
        base_params.remove("task_count");
        base_params.insert(
            "parallel_group".to_string(),
            serde_json::Value::String(task.task_name.clone()),
        );

        let mut replacements = Vec::with_capacity(count);
        for (index, item) in items.iter().enumerate() {
            let mut params = base_params.clone();
            params.insert("parallel_index".to_string(), serde_json::json!(index));
            params.insert("parallel_count".to_string(), serde_json::json!(count));
            params.insert("foreach_index".to_string(), serde_json::json!(index));
            params.insert("foreach_count".to_string(), serde_json::json!(count));
            params.insert("foreach_item".to_string(), item.clone());
            replacements.push(NewTask {
                task_index: if index == 0 {
                    task.task_index
                } else {
                    max_index + index as i32
                },
                task_name: format!("{}[{}]", task.task_name, index),
                executor_type: task.executor_type.clone(),
                depends_on: task.depends_on.clone(),
                params: serde_json::Value::Object(params),
                max_retries: task.max_retries,
                timeout_seconds: task.timeout_seconds,
            });
        }
        Ok(replacements)
    }
}
