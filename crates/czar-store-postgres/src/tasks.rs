use super::core::PostgresDatabase;
use anyhow::Result;
use chrono::{DateTime, Utc};
use czar_core::database::{NewTask, TaskRetryMeta};
use czar_core::models::{RunStatus, Task, TaskStatus, TaskWithWorkflow, WorkflowTaskState};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

impl PostgresDatabase {
    pub(super) async fn batch_create_tasks_impl(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for i in 0..task_count {
            let params = serde_json::json!({"task_index": i, "run_id": run_id, "workflow_name": workflow_name});
            sqlx::query("INSERT INTO tasks (run_id, task_index, task_name, executor_type, depends_on, status, params) VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (run_id, task_index) DO NOTHING")
                .bind(run_id)
                .bind(i)
                .bind(format!("task_{}", i))
                .bind(executor_type)
                .bind(Vec::<String>::new())
                .bind(TaskStatus::Pending.as_str())
                .bind(params)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn batch_create_dag_tasks_impl(
        &self,
        run_id: Uuid,
        tasks: &[NewTask],
    ) -> Result<()> {
        self.batch_create_dag_tasks_multi_impl(&[(run_id, tasks)])
            .await
    }

    pub(super) async fn batch_create_dag_tasks_multi_impl(
        &self,
        runs: &[(Uuid, &[NewTask])],
    ) -> Result<()> {
        // Postgres limit: 65535 bind parameters per statement. With 10 params/row,
        // max rows = 6553. Use 6000 for a safe margin.
        const MAX_ROWS_PER_INSERT: usize = 6000;

        let flat: Vec<(Uuid, &NewTask)> = runs
            .iter()
            .flat_map(|(run_id, tasks)| tasks.iter().map(move |t| (*run_id, t)))
            .collect();

        if flat.is_empty() {
            return Ok(());
        }

        for chunk in flat.chunks(MAX_ROWS_PER_INSERT) {
            let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                "INSERT INTO tasks (run_id, task_index, task_name, executor_type, depends_on, status, attempts, max_retries, timeout_seconds, params) ",
            );
            builder.push_values(chunk.iter(), |mut b, (run_id, task)| {
                b.push_bind(*run_id)
                    .push_bind(task.task_index)
                    .push_bind(&task.task_name)
                    .push_bind(&task.executor_type)
                    .push_bind(&task.depends_on)
                    .push_bind(TaskStatus::Pending.as_str())
                    .push_bind(0i32)
                    .push_bind(task.max_retries)
                    .push_bind(task.timeout_seconds)
                    .push_bind(&task.params);
            });
            builder.push(" ON CONFLICT (run_id, task_index) DO NOTHING");
            builder.build().execute(&self.pool).await?;
        }
        Ok(())
    }
    pub(super) async fn update_task_status_impl(
        &self,
        task_id: Uuid,
        status: TaskStatus,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let status_str = status.as_str();
        sqlx::query(
            r#"UPDATE tasks SET status = $1, execution_name = COALESCE($2, execution_name), error = $3,
            attempts = CASE WHEN $1 = $5 THEN attempts + 1 ELSE attempts END,
            retry_at = CASE WHEN $1 IN ($6, $7) THEN retry_at ELSE NULL END,
            dispatched_at = COALESCE(dispatched_at, CASE WHEN $1 = $8 THEN NOW() ELSE NULL END),
            started_at = COALESCE(started_at, CASE WHEN $1 IN ($9, $10, $5, $11) THEN NOW() ELSE NULL END),
            finished_at = CASE WHEN $1 IN ($10, $5, $11) THEN NOW() ELSE NULL END
            WHERE id = $4"#
        )
        .bind(status_str)
        .bind(execution_name)
        .bind(error)
        .bind(task_id)
        .bind(TaskStatus::Failed.as_str())
        .bind(TaskStatus::Pending.as_str())
        .bind(TaskStatus::Paused.as_str())
        .bind(TaskStatus::Dispatched.as_str())
        .bind(TaskStatus::Running.as_str())
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Cancelled.as_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    pub(super) async fn batch_update_task_status_impl(
        &self,
        updates: &[(Uuid, TaskStatus, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let ids: Vec<Uuid> = updates.iter().map(|(id, _, _, _)| *id).collect();
        let statuses: Vec<&str> = updates.iter().map(|(_, s, _, _)| s.as_str()).collect();
        let execution_names: Vec<Option<&str>> = updates.iter().map(|(_, _, e, _)| *e).collect();
        let errors: Vec<Option<&str>> = updates.iter().map(|(_, _, _, e)| *e).collect();
        sqlx::query(
            r#"UPDATE tasks SET
               status = data.status,
               execution_name = COALESCE(data.execution_name, tasks.execution_name),
               error = data.error,
               attempts = CASE WHEN data.status = $1 THEN tasks.attempts + 1 ELSE tasks.attempts END,
               retry_at = CASE WHEN data.status IN ($2, $3) THEN tasks.retry_at ELSE NULL END,
               dispatched_at = COALESCE(tasks.dispatched_at, CASE WHEN data.status = $4 THEN NOW() ELSE NULL END),
               started_at = COALESCE(tasks.started_at, CASE WHEN data.status IN ($5, $6, $1, $7) THEN NOW() ELSE NULL END),
               finished_at = CASE WHEN data.status IN ($6, $1, $7) THEN NOW() ELSE NULL END
               FROM unnest($8::uuid[], $9::text[], $10::text[], $11::text[])
                 AS data(id, status, execution_name, error)
               WHERE tasks.id = data.id
                 AND (tasks.status NOT IN ($6, $1) OR data.status IN ($6, $1))"#,
        )
        .bind(TaskStatus::Failed.as_str())     // $1
        .bind(TaskStatus::Pending.as_str())    // $2
        .bind(TaskStatus::Paused.as_str())     // $3
        .bind(TaskStatus::Dispatched.as_str()) // $4
        .bind(TaskStatus::Running.as_str())    // $5
        .bind(TaskStatus::Success.as_str())    // $6
        .bind(TaskStatus::Cancelled.as_str())  // $7
        .bind(&ids)                            // $8
        .bind(&statuses)                       // $9
        .bind(&execution_names)                // $10
        .bind(&errors)                         // $11
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    pub(super) async fn list_tasks_impl(&self, run_id: Uuid) -> Result<Vec<Task>> {
        let tasks =
            sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE run_id = $1 ORDER BY task_index")
                .bind(run_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(tasks)
    }
    pub(super) async fn get_pending_tasks_impl(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE status = $1")
            .bind(TaskStatus::Pending.as_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(tasks)
    }
    pub(super) async fn get_running_tasks_impl(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE status IN ($1, $2)")
            .bind(TaskStatus::Running.as_str())
            .bind(TaskStatus::Dispatched.as_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(tasks)
    }
    pub(super) async fn batch_append_task_logs_impl(
        &self,
        entries: &[(Uuid, String)],
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let ids: Vec<Uuid> = entries.iter().map(|(id, _)| *id).collect();
        let chunks: Vec<&str> = entries.iter().map(|(_, c)| c.as_str()).collect();
        sqlx::query(
            r#"
            UPDATE tasks
            SET logs = COALESCE(SUBSTRING(logs FROM 1 FOR 2000000), '') || data.chunk
            FROM (
                SELECT unnest($1::uuid[]) AS task_id,
                       unnest($2::text[]) AS chunk
            ) AS data
            WHERE tasks.id = data.task_id
            "#,
        )
        .bind(&ids)
        .bind(&chunks)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    pub(super) async fn update_task_output_impl(
        &self,
        task_id: Uuid,
        output: serde_json::Value,
    ) -> Result<()> {
        sqlx::query("UPDATE tasks SET output = $1 WHERE id = $2")
            .bind(output)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub(super) async fn update_task_progress_impl(
        &self,
        task_id: Uuid,
        progress: serde_json::Value,
    ) -> Result<()> {
        sqlx::query("UPDATE tasks SET progress = $1 WHERE id = $2")
            .bind(progress)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub(super) async fn reset_task_for_retry_impl(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<chrono::DateTime<Utc>>,
    ) -> Result<()> {
        sqlx::query("UPDATE tasks SET status = $1, attempts = attempts + 1, retry_at = $2, error = COALESCE($3, error), execution_name = NULL, output = NULL, dispatched_at = NULL, started_at = NULL, finished_at = NULL WHERE id = $4")
            .bind(TaskStatus::Pending.as_str())
            .bind(retry_at)
            .bind(error)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub(super) async fn get_pending_tasks_with_workflow_impl(
        &self,
        limit: i64,
    ) -> Result<Vec<TaskWithWorkflow>> {
        let tasks = sqlx::query_as::<_, TaskWithWorkflow>(
            r#"SELECT t.id as task_id, t.run_id, t.task_index, t.task_name, t.executor_type, t.depends_on,
            t.status as task_status, t.attempts, t.max_retries, t.timeout_seconds, t.retry_at, t.execution_name, t.params,
            w.id as workflow_id, w.job_name, w.project, w.region
            FROM tasks t JOIN runs r ON t.run_id = r.id JOIN workflows w ON r.workflow_id = w.id
            WHERE t.status = $2
            AND r.status = $3
            AND (t.retry_at IS NULL OR t.retry_at <= NOW())
            AND (cardinality(t.depends_on) = 0 OR NOT EXISTS (
                SELECT 1 FROM unnest(t.depends_on) AS dep_name
                JOIN tasks t2
                  ON t2.run_id = t.run_id
                 AND t2.task_name = dep_name
                WHERE t2.status NOT IN ($4, $5, $6)
            ))
            ORDER BY t.created_at LIMIT $1"#
        )
        .bind(limit)
        .bind(TaskStatus::Pending.as_str())
        .bind(RunStatus::Running.as_str())
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Failed.as_str())
        .bind(TaskStatus::Skipped.as_str())
        .fetch_all(&self.pool)
        .await?;
        Ok(tasks)
    }
    pub(super) async fn get_task_outputs_impl(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        if task_names.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query("SELECT task_name, output FROM tasks WHERE run_id = $1 AND task_name = ANY($2) AND output IS NOT NULL")
            .bind(run_id).bind(task_names).fetch_all(&self.pool).await?;
        let mut map = HashMap::new();
        for row in rows {
            let name: String = row.get("task_name");
            let output: serde_json::Value = row.get("output");
            map.insert(name, output);
        }
        Ok(map)
    }
    pub(super) async fn get_task_retry_meta_impl(
        &self,
        task_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, TaskRetryMeta>> {
        if task_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows =
            sqlx::query("SELECT id, attempts, max_retries, params FROM tasks WHERE id = ANY($1)")
                .bind(task_ids)
                .fetch_all(&self.pool)
                .await?;
        let mut map = HashMap::new();
        for row in rows {
            let id: Uuid = row.get("id");
            let attempts: i32 = row.get("attempts");
            let max_retries: i32 = row.get("max_retries");
            let params: Option<sqlx::types::Json<serde_json::Value>> = row.try_get("params")?;
            let retry_delay_seconds = params
                .as_ref()
                .and_then(|value| value.0.get("retry_delay_seconds"))
                .and_then(|value| value.as_i64())
                .map(|value| value.clamp(0, i64::from(i32::MAX)) as i32);
            map.insert(
                id,
                TaskRetryMeta {
                    attempts,
                    max_retries,
                    retry_delay_seconds,
                },
            );
        }
        Ok(map)
    }
    pub(super) async fn get_task_run_id_impl(&self, task_id: Uuid) -> Result<Uuid> {
        let row = sqlx::query("SELECT run_id FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get("run_id"))
    }
    pub(super) async fn get_task_identity_impl(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        let row = sqlx::query("SELECT run_id, task_name FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?;
        Ok((row.get("run_id"), row.get("task_name")))
    }
    pub(super) async fn mark_tasks_failed_by_dependency_impl(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> Result<Vec<String>> {
        if failed_task_names.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query("UPDATE tasks SET status = $1, error = $2, finished_at = NOW() WHERE run_id = $3 AND status IN ($4, $5) AND depends_on && $6 RETURNING task_name")
            .bind(TaskStatus::Failed.as_str())
            .bind(error)
            .bind(run_id)
            .bind(TaskStatus::Pending.as_str())
            .bind(TaskStatus::Paused.as_str())
            .bind(failed_task_names)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|row| row.get("task_name")).collect())
    }

    pub(super) async fn upsert_workflow_task_state_impl(
        &self,
        workflow_id: Uuid,
        task_name: &str,
        last_status: TaskStatus,
        last_run_id: Uuid,
        last_task_id: Uuid,
        last_output: Option<serde_json::Value>,
        last_error: Option<&str>,
        last_finished_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let status_str = last_status.as_str();
        sqlx::query(
            r#"INSERT INTO workflow_task_states (
                workflow_id, task_name, last_status, last_run_id, last_task_id,
                last_output, last_error, last_finished_at, last_success_at, last_failure_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8,
                CASE WHEN $3 = 'success' THEN $8 ELSE NULL END,
                CASE WHEN $3 IN ('failed', 'cancelled') THEN $8 ELSE NULL END,
                NOW()
            )
            ON CONFLICT (workflow_id, task_name) DO UPDATE SET
                last_status = EXCLUDED.last_status,
                last_run_id = EXCLUDED.last_run_id,
                last_task_id = EXCLUDED.last_task_id,
                last_output = EXCLUDED.last_output,
                last_error = EXCLUDED.last_error,
                last_finished_at = EXCLUDED.last_finished_at,
                last_success_at = CASE
                    WHEN EXCLUDED.last_status = 'success' THEN EXCLUDED.last_finished_at
                    ELSE workflow_task_states.last_success_at
                END,
                last_failure_at = CASE
                    WHEN EXCLUDED.last_status IN ('failed', 'cancelled') THEN EXCLUDED.last_finished_at
                    ELSE workflow_task_states.last_failure_at
                END,
                updated_at = NOW()"#,
        )
        .bind(workflow_id)
        .bind(task_name)
        .bind(status_str)
        .bind(last_run_id)
        .bind(last_task_id)
        .bind(last_output)
        .bind(last_error)
        .bind(last_finished_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn batch_upsert_workflow_task_states_impl(
        &self,
        entries: &[czar_core::database::WorkflowTaskStateEntry],
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let now = chrono::Utc::now();
        let workflow_ids: Vec<Uuid> = entries.iter().map(|e| e.workflow_id).collect();
        let task_names: Vec<&str> = entries.iter().map(|e| e.task_name.as_str()).collect();
        let statuses: Vec<&str> = entries.iter().map(|e| e.last_status.as_str()).collect();
        let run_ids: Vec<Uuid> = entries.iter().map(|e| e.last_run_id).collect();
        let task_ids: Vec<Uuid> = entries.iter().map(|e| e.last_task_id).collect();
        let outputs: Vec<Option<serde_json::Value>> =
            entries.iter().map(|e| e.last_output.clone()).collect();
        let errors: Vec<Option<&str>> = entries.iter().map(|e| e.last_error.as_deref()).collect();
        let finished_ats: Vec<Option<chrono::DateTime<chrono::Utc>>> =
            entries.iter().map(|e| e.last_finished_at).collect();
        let updated_ats: Vec<chrono::DateTime<chrono::Utc>> = entries.iter().map(|_| now).collect();

        sqlx::query(
            r#"INSERT INTO workflow_task_states (
                workflow_id, task_name, last_status, last_run_id, last_task_id,
                last_output, last_error, last_finished_at, last_success_at, last_failure_at, updated_at
            )
            SELECT
                workflow_id, task_name, last_status, last_run_id, last_task_id,
                last_output, last_error, last_finished_at,
                CASE WHEN last_status = 'success' THEN last_finished_at ELSE NULL END,
                CASE WHEN last_status IN ('failed', 'cancelled') THEN last_finished_at ELSE NULL END,
                updated_at
            FROM unnest(
                $1::uuid[], $2::text[], $3::text[], $4::uuid[], $5::uuid[],
                $6::jsonb[], $7::text[], $8::timestamptz[], $9::timestamptz[]
            ) AS data(workflow_id, task_name, last_status, last_run_id, last_task_id,
                       last_output, last_error, last_finished_at, updated_at)
            ON CONFLICT (workflow_id, task_name) DO UPDATE SET
                last_status = EXCLUDED.last_status,
                last_run_id = EXCLUDED.last_run_id,
                last_task_id = EXCLUDED.last_task_id,
                last_output = EXCLUDED.last_output,
                last_error = EXCLUDED.last_error,
                last_finished_at = EXCLUDED.last_finished_at,
                last_success_at = CASE
                    WHEN EXCLUDED.last_status = 'success' THEN EXCLUDED.last_finished_at
                    ELSE workflow_task_states.last_success_at
                END,
                last_failure_at = CASE
                    WHEN EXCLUDED.last_status IN ('failed', 'cancelled') THEN EXCLUDED.last_finished_at
                    ELSE workflow_task_states.last_failure_at
                END,
                updated_at = EXCLUDED.updated_at"#,
        )
        .bind(&workflow_ids)
        .bind(&task_names)
        .bind(&statuses)
        .bind(&run_ids)
        .bind(&task_ids)
        .bind(&outputs)
        .bind(&errors)
        .bind(&finished_ats)
        .bind(&updated_ats)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn list_workflow_task_states_impl(
        &self,
        workflow_id: Uuid,
    ) -> Result<Vec<WorkflowTaskState>> {
        let states = sqlx::query_as::<_, WorkflowTaskState>(
            "SELECT * FROM workflow_task_states WHERE workflow_id = $1 ORDER BY task_name",
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(states)
    }
}
