use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use czar_core::database::{RunListEntry, RunListPage, RunListQuery, RunRepository};
use czar_core::models::{Run, RunStatus};

use crate::client::QueryParam;
use crate::database::{
    D1Database, decode, opt_json, opt_text, opt_uuid, scalar_from_row, uuid_param,
};

impl D1Database {
    pub(crate) async fn create_run_impl(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
    ) -> Result<Run> {
        self.create_run_with_snapshot_and_selection_impl(workflow_id, None, triggered_by, None)
            .await
    }

    pub(crate) async fn create_run_with_selection_impl(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
        task_selection: Option<Value>,
    ) -> Result<Run> {
        self.create_run_with_snapshot_and_selection_impl(
            workflow_id,
            None,
            triggered_by,
            task_selection,
        )
        .await
    }

    pub(crate) async fn create_run_with_snapshot_and_selection_impl(
        &self,
        workflow_id: Uuid,
        snapshot_id: Option<Uuid>,
        triggered_by: &str,
        task_selection: Option<Value>,
    ) -> Result<Run> {
        let snapshot_id = match snapshot_id {
            Some(snapshot_id) => Some(snapshot_id),
            None => {
                self.get_workflow_by_id_impl(workflow_id)
                    .await?
                    .current_snapshot_id
            }
        };

        self.fetch_one(
            r#"INSERT INTO runs (
                id, workflow_id, snapshot_id, task_selection, status, triggered_by
            ) VALUES (?, ?, ?, ?, ?, ?) RETURNING *"#,
            vec![
                uuid_param(Uuid::new_v4()),
                uuid_param(workflow_id),
                opt_uuid(snapshot_id),
                opt_json(task_selection),
                RunStatus::Pending.as_str().into(),
                triggered_by.into(),
            ],
            decode::run,
        )
        .await
    }

    pub(crate) async fn update_run_status_impl(
        &self,
        run_id: Uuid,
        status: RunStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let status_str = status.as_str();
        self.execute(
            r#"UPDATE runs SET status = ?, error = ?,
               started_at = CASE WHEN ? = ? AND started_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END,
               finished_at = CASE WHEN ? IN (?, ?, ?) AND finished_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE finished_at END
               WHERE id = ?"#,
            vec![
                status_str.into(),
                opt_text(error),
                status_str.into(),
                RunStatus::Running.as_str().into(),
                status_str.into(),
                RunStatus::Success.as_str().into(),
                RunStatus::Failed.as_str().into(),
                RunStatus::Cancelled.as_str().into(),
                uuid_param(run_id),
            ],
        )
        .await
    }

    pub(crate) async fn get_run_impl(&self, run_id: Uuid) -> Result<Run> {
        self.fetch_one(
            "SELECT * FROM runs WHERE id = ?",
            vec![uuid_param(run_id)],
            decode::run,
        )
        .await
    }

    pub(crate) async fn list_runs_impl(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        match workflow_id {
            Some(workflow_id) => {
                self.fetch_all(
                    "SELECT * FROM runs WHERE workflow_id = ? ORDER BY created_at DESC",
                    vec![uuid_param(workflow_id)],
                    decode::run,
                )
                .await
            }
            None => {
                self.fetch_all(
                    "SELECT * FROM runs ORDER BY created_at DESC",
                    Vec::new(),
                    decode::run,
                )
                .await
            }
        }
    }

    pub(crate) async fn list_runs_page_impl(&self, query: &RunListQuery) -> Result<RunListPage> {
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
        let has_status = !status_strs.is_empty();
        let has_workflow = !query.workflow_names.is_empty();

        // Build WHERE clause
        let mut where_parts: Vec<String> = Vec::new();
        let mut all_params: Vec<QueryParam> = Vec::new();

        if let Some(search_pattern) = search_pattern.as_ref() {
            where_parts.push(
                "(LOWER(COALESCE(w.name, '')) LIKE ? OR LOWER(CAST(r.id AS TEXT)) LIKE ?)"
                    .to_string(),
            );
            all_params.push(search_pattern.as_str().into());
            all_params.push(search_pattern.as_str().into());
        }
        if has_status {
            let placeholders = status_strs
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");
            where_parts.push(format!("r.status IN ({})", placeholders));
            for s in &status_strs {
                all_params.push(s.as_str().into());
            }
        }
        if has_workflow {
            let placeholders = query
                .workflow_names
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");
            where_parts.push(format!("w.name IN ({})", placeholders));
            for n in &query.workflow_names {
                all_params.push(n.as_str().into());
            }
        }
        if let Some(since) = query.since {
            where_parts.push("r.created_at >= ?".to_string());
            all_params.push(since.to_rfc3339().into());
        }
        if let Some(until) = query.until {
            where_parts.push("r.created_at <= ?".to_string());
            all_params.push(until.to_rfc3339().into());
        }
        let where_clause = if where_parts.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_parts.join(" AND "))
        };

        // Count
        let count_sql = format!(
            "SELECT COUNT(*) AS total FROM runs r LEFT JOIN workflows w ON w.id = r.workflow_id{}",
            where_clause
        );
        let total = self
            .scalar_i64(&count_sql, all_params.clone(), "total")
            .await?;

        // List
        let mut list_params = all_params;
        list_params.push(query.limit.into());
        list_params.push(query.offset.into());
        let list_sql = format!(
            r#"SELECT r.*, w.name AS workflow_name FROM runs r LEFT JOIN workflows w ON w.id = r.workflow_id{} ORDER BY r.created_at DESC, r.id DESC LIMIT ? OFFSET ?"#,
            where_clause
        );
        let items = self
            .fetch_all(&list_sql, list_params, |row| {
                let workflow_name = decode::opt_string(&row, "workflow_name")?;
                let run = decode::run(row)?;
                Ok(RunListEntry { run, workflow_name })
            })
            .await?;

        Ok(RunListPage {
            items,
            total: total as usize,
        })
    }

    pub(crate) async fn get_pending_runs_impl(&self) -> Result<Vec<Run>> {
        self.fetch_all(
            "SELECT * FROM runs WHERE status = ?",
            vec![RunStatus::Pending.as_str().into()],
            decode::run,
        )
        .await
    }

    pub(crate) async fn cancel_run_impl(&self, run_id: Uuid) -> Result<()> {
        self.execute(
            "UPDATE runs SET status = ?, finished_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND status NOT IN (?, ?, ?)",
            vec![
                RunStatus::Cancelled.as_str().into(),
                uuid_param(run_id),
                RunStatus::Success.as_str().into(),
                RunStatus::Failed.as_str().into(),
                RunStatus::Cancelled.as_str().into(),
            ],
        )
        .await?;
        self.execute(
            "UPDATE tasks SET status = ?, finished_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') WHERE run_id = ? AND status IN (?, ?, ?)",
            vec![
                czar_core::models::TaskStatus::Cancelled.as_str().into(),
                uuid_param(run_id),
                czar_core::models::TaskStatus::Pending.as_str().into(),
                czar_core::models::TaskStatus::Dispatched.as_str().into(),
                czar_core::models::TaskStatus::Running.as_str().into(),
            ],
        )
        .await
    }

    pub(crate) async fn get_run_task_stats_impl(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        let row = self
            .fetch_one(
                r#"SELECT COUNT(*) AS total,
                   COUNT(CASE WHEN status IN (?, ?) THEN 1 END) AS completed,
                   COUNT(CASE WHEN status = ? THEN 1 END) AS failed
                   FROM tasks WHERE run_id = ?"#,
                vec![
                    czar_core::models::TaskStatus::Success.as_str().into(),
                    czar_core::models::TaskStatus::Failed.as_str().into(),
                    czar_core::models::TaskStatus::Failed.as_str().into(),
                    uuid_param(run_id),
                ],
                Ok,
            )
            .await?;

        Ok((
            scalar_from_row(&row, "total")?,
            scalar_from_row(&row, "completed")?,
            scalar_from_row(&row, "failed")?,
        ))
    }
}

#[async_trait]
impl RunRepository for D1Database {
    async fn create_run(&self, workflow_id: Uuid, triggered_by: &str) -> Result<Run> {
        self.create_run_impl(workflow_id, triggered_by).await
    }

    async fn create_run_with_snapshot_and_selection(
        &self,
        workflow_id: Uuid,
        snapshot_id: Option<Uuid>,
        triggered_by: &str,
        task_selection: Option<Value>,
    ) -> Result<Run> {
        self.create_run_with_snapshot_and_selection_impl(
            workflow_id,
            snapshot_id,
            triggered_by,
            task_selection,
        )
        .await
    }

    async fn create_run_with_selection(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
        task_selection: Option<Value>,
    ) -> Result<Run> {
        self.create_run_with_selection_impl(workflow_id, triggered_by, task_selection)
            .await
    }

    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: RunStatus,
        error: Option<&str>,
    ) -> Result<()> {
        self.update_run_status_impl(run_id, status, error).await
    }

    async fn get_run(&self, run_id: Uuid) -> Result<Run> {
        self.get_run_impl(run_id).await
    }

    async fn list_runs(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        self.list_runs_impl(workflow_id).await
    }

    async fn list_runs_page(&self, query: &RunListQuery) -> Result<RunListPage> {
        self.list_runs_page_impl(query).await
    }

    async fn get_pending_runs(&self) -> Result<Vec<Run>> {
        self.get_pending_runs_impl().await
    }

    async fn cancel_run(&self, run_id: Uuid) -> Result<()> {
        self.cancel_run_impl(run_id).await
    }

    async fn get_run_task_stats(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        self.get_run_task_stats_impl(run_id).await
    }
}
