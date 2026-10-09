use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use czar_core::database::{NewWorkflowTask, WorkflowListPage, WorkflowListQuery};
use czar_core::models::{Workflow, WorkflowSnapshot, WorkflowTask};

use super::core::{SqliteDatabase, WorkflowSnapshotRow, WorkflowTaskRow, encode_depends_on};

impl SqliteDatabase {
    pub(super) async fn create_workflow_impl(
        &self,
        name: &str,
        description: Option<&str>,
        job_name: &str,
        region: &str,
        project: &str,
        executor_type: &str,
        task_params: Option<serde_json::Value>,
        schedule: Option<&str>,
    ) -> Result<Workflow> {
        let workflow_id = Uuid::new_v4();
        let params_json = task_params.map(sqlx::types::Json);
        let workflow = sqlx::query_as::<_, Workflow>(
            r#"INSERT INTO workflows (id, name, description, job_name, region, project, executor_type, task_params, schedule)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"#,
        )
        .bind(workflow_id).bind(name).bind(description).bind(job_name).bind(region).bind(project).bind(executor_type).bind(params_json).bind(schedule)
        .fetch_one(&self.pool).await?;
        Ok(workflow)
    }

    pub(super) async fn get_workflow_impl(&self, name: &str) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE name = ?")
            .bind(name)
            .fetch_one(&self.pool)
            .await?;
        Ok(workflow)
    }

    pub(super) async fn list_workflows_impl(&self) -> Result<Vec<Workflow>> {
        let workflows =
            sqlx::query_as::<_, Workflow>("SELECT * FROM workflows ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await?;
        Ok(workflows)
    }

    pub(super) async fn list_workflows_page_impl(
        &self,
        query: &WorkflowListQuery,
    ) -> Result<WorkflowListPage> {
        let limit = i64::try_from(query.limit).unwrap_or(i64::MAX);
        let offset = i64::try_from(query.offset).unwrap_or(i64::MAX);
        let search = query
            .search
            .as_ref()
            .map(|s| format!("%{}%", s.to_lowercase()));

        let (total, items) = match search.as_deref() {
            Some(pattern) => {
                let total = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM workflows WHERE LOWER(name) LIKE ?",
                )
                .bind(pattern)
                .fetch_one(&self.pool)
                .await?;

                let items = sqlx::query_as::<_, Workflow>(
                    "SELECT * FROM workflows WHERE LOWER(name) LIKE ? ORDER BY name ASC LIMIT ? OFFSET ?",
                )
                .bind(pattern)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;
                (total, items)
            }
            None => {
                let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows")
                    .fetch_one(&self.pool)
                    .await?;

                let items = sqlx::query_as::<_, Workflow>(
                    "SELECT * FROM workflows ORDER BY name ASC LIMIT ? OFFSET ?",
                )
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;
                (total, items)
            }
        };

        Ok(WorkflowListPage {
            items,
            total: total as usize,
        })
    }

    pub(super) async fn delete_workflow_impl(&self, name: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM runs WHERE workflow_id = (SELECT id FROM workflows WHERE name = ?)",
        )
        .bind(name)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM workflows WHERE name = ?")
            .bind(name)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn get_workflows_by_ids_impl(
        &self,
        workflow_ids: &[Uuid],
    ) -> Result<Vec<Workflow>> {
        if workflow_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = workflow_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let query_str = format!("SELECT * FROM workflows WHERE id IN ({})", placeholders);
        let mut query = sqlx::query_as::<_, Workflow>(sqlx::AssertSqlSafe(query_str));
        for id in workflow_ids {
            query = query.bind(id);
        }
        let workflows = query.fetch_all(&self.pool).await?;
        Ok(workflows)
    }

    pub(super) async fn get_workflow_by_id_impl(&self, workflow_id: Uuid) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE id = ?")
            .bind(workflow_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(workflow)
    }

    pub(super) async fn create_workflow_tasks_impl(
        &self,
        workflow_id: Uuid,
        tasks: &[NewWorkflowTask],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM workflow_tasks WHERE workflow_id = ?")
            .bind(workflow_id)
            .execute(&mut *tx)
            .await?;
        for task in tasks {
            let task_id = Uuid::new_v4();
            let depends_on_json = encode_depends_on(&task.depends_on);
            let params_json = sqlx::types::Json(&task.params);
            let signature_json = task.signature.as_ref().map(sqlx::types::Json);
            sqlx::query(
                r#"INSERT INTO workflow_tasks (id, workflow_id, task_index, task_name, executor_type, depends_on, params, signature)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
            )
            .bind(task_id)
            .bind(workflow_id)
            .bind(task.task_index)
            .bind(&task.task_name)
            .bind(&task.executor_type)
            .bind(&depends_on_json)
            .bind(params_json)
            .bind(signature_json)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn update_workflow_task_params_impl(
        &self,
        workflow_id: Uuid,
        task_params: Option<serde_json::Value>,
    ) -> Result<()> {
        let params_json = task_params.map(sqlx::types::Json);
        sqlx::query(
            "UPDATE workflows SET task_params = ?, updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        )
        .bind(params_json)
        .bind(workflow_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn list_workflow_tasks_impl(
        &self,
        workflow_id: Uuid,
    ) -> Result<Vec<WorkflowTask>> {
        let rows = sqlx::query_as::<_, WorkflowTaskRow>(
            "SELECT * FROM workflow_tasks WHERE workflow_id = ? ORDER BY task_index",
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Self::map_workflow_task).collect())
    }

    pub(super) async fn get_due_scheduled_workflows_impl(&self) -> Result<Vec<Workflow>> {
        let now = chrono::Utc::now();
        let workflows = sqlx::query_as::<_, Workflow>(
            r#"SELECT * FROM workflows
               WHERE schedule_enabled = 1
               AND schedule IS NOT NULL
               AND (next_scheduled_at IS NULL OR next_scheduled_at <= ?)
               ORDER BY next_scheduled_at"#,
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        Ok(workflows)
    }

    pub(super) async fn update_workflow_schedule_times_impl(
        &self,
        workflow_id: Uuid,
        last_scheduled_at: chrono::DateTime<chrono::Utc>,
        next_scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE workflows SET last_scheduled_at = ?, next_scheduled_at = ? WHERE id = ?",
        )
        .bind(last_scheduled_at)
        .bind(next_scheduled_at)
        .bind(workflow_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn update_workflow_schedule_impl(
        &self,
        workflow_id: Uuid,
        schedule: Option<&str>,
        schedule_enabled: bool,
    ) -> Result<()> {
        sqlx::query("UPDATE workflows SET schedule = ?, schedule_enabled = ? WHERE id = ?")
            .bind(schedule)
            .bind(schedule_enabled)
            .bind(workflow_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(super) async fn create_or_get_snapshot_impl(
        &self,
        workflow_id: Uuid,
        content_hash: &str,
        tasks_json: serde_json::Value,
        source_yaml: Option<String>,
    ) -> Result<WorkflowSnapshot> {
        let snapshot_id = Uuid::new_v4();
        let now = Utc::now();
        let tasks_json_str = serde_json::to_string(&tasks_json)?;

        let row = sqlx::query_as::<_, WorkflowSnapshotRow>(
            r#"INSERT INTO workflow_snapshots (id, workflow_id, content_hash, tasks_json, source_yaml, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT (workflow_id, content_hash)
            DO UPDATE SET source_yaml = COALESCE(excluded.source_yaml, workflow_snapshots.source_yaml)
            RETURNING *"#,
        )
        .bind(snapshot_id)
        .bind(workflow_id)
        .bind(content_hash)
        .bind(tasks_json_str)
        .bind(source_yaml)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Self::map_workflow_snapshot(row)
    }

    pub(super) async fn get_snapshot_impl(&self, snapshot_id: Uuid) -> Result<WorkflowSnapshot> {
        let row = sqlx::query_as::<_, WorkflowSnapshotRow>(
            "SELECT * FROM workflow_snapshots WHERE id = ?",
        )
        .bind(snapshot_id)
        .fetch_one(&self.pool)
        .await?;
        Self::map_workflow_snapshot(row)
    }

    pub(super) async fn update_workflow_snapshot_impl(
        &self,
        workflow_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<()> {
        sqlx::query("UPDATE workflows SET current_snapshot_id = ? WHERE id = ?")
            .bind(snapshot_id)
            .bind(workflow_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
