use super::core::PostgresDatabase;
use anyhow::Result;
use chrono::{DateTime, Utc};
use czar_core::database::{RunListEntry, RunListPage, RunListQuery};
use czar_core::models::{Run, RunStatus, TaskStatus};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Default, sqlx::FromRow)]
struct RunWithWorkflowRow {
    id: Uuid,
    workflow_id: Uuid,
    snapshot_id: Option<Uuid>,
    task_selection: Option<sqlx::types::Json<serde_json::Value>>,
    status: RunStatus,
    triggered_by: String,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    error: Option<String>,
    created_at: DateTime<Utc>,
    workflow_name: Option<String>,
}

impl PostgresDatabase {
    pub(super) async fn create_run_impl(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
    ) -> Result<Run> {
        self.create_run_with_snapshot_and_selection_impl(workflow_id, None, triggered_by, None)
            .await
    }

    pub(super) async fn create_run_with_selection_impl(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
        task_selection: Option<serde_json::Value>,
    ) -> Result<Run> {
        self.create_run_with_snapshot_and_selection_impl(
            workflow_id,
            None,
            triggered_by,
            task_selection,
        )
        .await
    }

    pub(super) async fn create_run_with_snapshot_and_selection_impl(
        &self,
        workflow_id: Uuid,
        snapshot_id: Option<Uuid>,
        triggered_by: &str,
        task_selection: Option<serde_json::Value>,
    ) -> Result<Run> {
        let snapshot_id = match snapshot_id {
            Some(snapshot_id) => Some(snapshot_id),
            None => {
                let workflow = self.get_workflow_by_id_impl(workflow_id).await?;
                workflow.current_snapshot_id
            }
        };
        let task_selection = task_selection.map(sqlx::types::Json);

        let run = sqlx::query_as::<_, Run>(
            "INSERT INTO runs (workflow_id, snapshot_id, task_selection, status, triggered_by) VALUES ($1, $2, $3, $4, $5) RETURNING *",
        )
        .bind(workflow_id)
        .bind(snapshot_id)
        .bind(task_selection)
        .bind(RunStatus::Pending.as_str())
        .bind(triggered_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(run)
    }
    pub(super) async fn update_run_status_impl(
        &self,
        run_id: Uuid,
        status: RunStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let status_str = status.as_str();
        sqlx::query("UPDATE runs SET status = $1, error = $2, started_at = COALESCE(started_at, CASE WHEN $1 = $3 THEN NOW() ELSE NULL END), finished_at = CASE WHEN $1 IN ($4, $5, $6) THEN NOW() ELSE NULL END WHERE id = $7")
            .bind(status_str)
            .bind(error)
            .bind(RunStatus::Running.as_str())
            .bind(RunStatus::Success.as_str())
            .bind(RunStatus::Failed.as_str())
            .bind(RunStatus::Cancelled.as_str())
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub(super) async fn get_run_impl(&self, run_id: Uuid) -> Result<Run> {
        let run = sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(run)
    }
    pub(super) async fn list_runs_impl(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        let runs = match workflow_id {
            Some(wf_id) => {
                sqlx::query_as::<_, Run>(
                    "SELECT * FROM runs WHERE workflow_id = $1 ORDER BY created_at DESC",
                )
                .bind(wf_id)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, Run>("SELECT * FROM runs ORDER BY created_at DESC")
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok(runs)
    }

    pub(super) async fn list_runs_page_impl(&self, query: &RunListQuery) -> Result<RunListPage> {
        let limit = i64::try_from(query.limit).unwrap_or(i64::MAX);
        let offset = i64::try_from(query.offset).unwrap_or(i64::MAX);

        let search_pattern = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("%{}%", value.to_ascii_lowercase()));
        let status_strs: Vec<String> = query
            .statuses
            .iter()
            .map(|s| s.as_str().to_string())
            .collect();
        let has_search = search_pattern.is_some();
        let has_status = !status_strs.is_empty();
        let has_workflow = !query.workflow_names.is_empty();
        let has_since = query.since.is_some();
        let has_any =
            has_search || has_workflow || has_status || has_since || query.until.is_some();

        macro_rules! push_filters {
            ($b:expr) => {{
                let mut need_and = false;
                if has_any {
                    $b.push(" WHERE ");
                }
                if let Some(search_pattern) = search_pattern.as_ref() {
                    $b.push("(LOWER(COALESCE(w.name, '')) LIKE ");
                    $b.push_bind(search_pattern.clone());
                    $b.push(" OR LOWER(CAST(r.id AS TEXT)) LIKE ");
                    $b.push_bind(search_pattern.clone());
                    $b.push(")");
                    need_and = true;
                }
                if has_status {
                    if need_and {
                        $b.push(" AND ");
                    }
                    $b.push("r.status IN (");
                    let mut sep = $b.separated(", ");
                    for s in &status_strs {
                        sep.push_bind(s.clone());
                    }
                    $b.push(")");
                    need_and = true;
                }
                if has_workflow {
                    if need_and {
                        $b.push(" AND ");
                    }
                    $b.push("w.name IN (");
                    let mut sep = $b.separated(", ");
                    for n in &query.workflow_names {
                        sep.push_bind(n.clone());
                    }
                    $b.push(")");
                    need_and = true;
                }
                if let Some(since) = query.since {
                    if need_and {
                        $b.push(" AND ");
                    }
                    $b.push("r.created_at >= ");
                    $b.push_bind(since);
                    need_and = true;
                }
                if let Some(until) = query.until {
                    if need_and {
                        $b.push(" AND ");
                    }
                    $b.push("r.created_at <= ");
                    $b.push_bind(until);
                }
            }};
        }

        // count
        let total: i64 = {
            let mut b = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                "SELECT COUNT(*) FROM runs r LEFT JOIN workflows w ON w.id = r.workflow_id",
            );
            push_filters!(b);
            b.build_query_scalar::<i64>().fetch_one(&self.pool).await?
        };

        // rows
        let rows: Vec<RunWithWorkflowRow> = {
            let mut b = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                r#"SELECT r.id, r.workflow_id, r.snapshot_id, r.status, r.triggered_by, r.started_at, r.finished_at, r.error, r.created_at, r.task_selection, w.name AS workflow_name
                   FROM runs r LEFT JOIN workflows w ON w.id = r.workflow_id"#,
            );
            push_filters!(b);
            b.push(" ORDER BY r.created_at DESC, r.id DESC LIMIT ");
            b.push_bind(limit);
            b.push(" OFFSET ");
            b.push_bind(offset);
            b.build_query_as::<RunWithWorkflowRow>()
                .fetch_all(&self.pool)
                .await?
        };

        let items = rows
            .into_iter()
            .map(|row| RunListEntry {
                run: Run {
                    id: row.id,
                    workflow_id: row.workflow_id,
                    snapshot_id: row.snapshot_id,
                    task_selection: row.task_selection,
                    status: row.status,
                    triggered_by: row.triggered_by,
                    started_at: row.started_at,
                    finished_at: row.finished_at,
                    error: row.error,
                    created_at: row.created_at,
                },
                workflow_name: row.workflow_name,
            })
            .collect();

        Ok(RunListPage {
            items,
            total: total as usize,
        })
    }
    pub(super) async fn get_pending_runs_impl(&self) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE status = $1")
            .bind(RunStatus::Pending.as_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(runs)
    }
    pub(super) async fn cancel_run_impl(&self, run_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE runs SET status = $1, finished_at = NOW() WHERE id = $2 AND status NOT IN ($3, $4, $5)",
        )
            .bind(RunStatus::Cancelled.as_str())
            .bind(run_id)
            .bind(RunStatus::Success.as_str())
            .bind(RunStatus::Failed.as_str())
            .bind(RunStatus::Cancelled.as_str())
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE tasks SET status = $1, finished_at = NOW() WHERE run_id = $2 AND status IN ($3, $4, $5)")
            .bind(TaskStatus::Cancelled.as_str())
            .bind(run_id)
            .bind(TaskStatus::Pending.as_str())
            .bind(TaskStatus::Dispatched.as_str())
            .bind(TaskStatus::Running.as_str())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn get_run_task_stats_impl(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        let row = sqlx::query("SELECT COUNT(*) as total, COUNT(*) FILTER (WHERE status IN ($1, $2, $3)) as completed, COUNT(*) FILTER (WHERE status = $2) as failed FROM tasks WHERE run_id = $4")
            .bind(TaskStatus::Success.as_str())
            .bind(TaskStatus::Failed.as_str())
            .bind(TaskStatus::Skipped.as_str())
            .bind(run_id)
            .fetch_one(&self.pool)
            .await?;
        Ok((row.get("total"), row.get("completed"), row.get("failed")))
    }

    pub(super) async fn batch_get_run_task_stats_impl(
        &self,
        run_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, (i64, i64, i64)>> {
        if run_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query(
            "SELECT run_id, \
             COUNT(*) as total, \
             COUNT(*) FILTER (WHERE status IN ($1, $2, $3)) as completed, \
             COUNT(*) FILTER (WHERE status = $2) as failed \
             FROM tasks WHERE run_id = ANY($4) GROUP BY run_id",
        )
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Failed.as_str())
        .bind(TaskStatus::Skipped.as_str())
        .bind(run_ids)
        .fetch_all(&self.pool)
        .await?;
        let mut result = HashMap::new();
        for row in rows {
            let run_id: Uuid = row.get("run_id");
            result.insert(
                run_id,
                (row.get("total"), row.get("completed"), row.get("failed")),
            );
        }
        Ok(result)
    }

    pub(super) async fn batch_update_run_statuses_impl(
        &self,
        updates: &[(Uuid, RunStatus, Option<String>)],
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let ids: Vec<Uuid> = updates.iter().map(|(id, _, _)| *id).collect();
        let statuses: Vec<&str> = updates.iter().map(|(_, s, _)| s.as_str()).collect();
        let errors: Vec<Option<&str>> = updates.iter().map(|(_, _, e)| e.as_deref()).collect();
        sqlx::query(
            r#"UPDATE runs SET
               status = data.status,
               error = data.error,
               started_at = COALESCE(runs.started_at, CASE WHEN data.status = $1 THEN NOW() ELSE NULL END),
               finished_at = CASE WHEN data.status IN ($2, $3, $4) THEN NOW() ELSE NULL END
               FROM unnest($5::uuid[], $6::text[], $7::text[]) AS data(id, status, error)
               WHERE runs.id = data.id"#,
        )
        .bind(RunStatus::Running.as_str())
        .bind(RunStatus::Success.as_str())
        .bind(RunStatus::Failed.as_str())
        .bind(RunStatus::Cancelled.as_str())
        .bind(&ids)
        .bind(&statuses)
        .bind(&errors)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
