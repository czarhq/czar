use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use czar_core::database::{
    NewWorkflowTask, WorkflowListPage, WorkflowListQuery, WorkflowRepository,
    WorkflowSnapshotRepository,
};
use czar_core::models::{Workflow, WorkflowSnapshot, WorkflowTask};

use crate::database::{D1Database, decode, opt_json, opt_text, opt_text_owned, uuid_param};

impl D1Database {
    pub(crate) async fn create_workflow_impl(
        &self,
        name: &str,
        description: Option<&str>,
        job_name: &str,
        region: &str,
        project: &str,
        executor_type: &str,
        task_params: Option<Value>,
        schedule: Option<&str>,
    ) -> Result<Workflow> {
        self.fetch_one(
            r#"INSERT INTO workflows (
                id, name, description, job_name, region, project, executor_type, task_params, schedule
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"#,
            vec![
                uuid_param(Uuid::new_v4()),
                name.into(),
                opt_text(description),
                job_name.into(),
                region.into(),
                project.into(),
                executor_type.into(),
                opt_json(task_params),
                opt_text(schedule),
            ],
            decode::workflow,
        )
        .await
    }

    pub(crate) async fn get_workflow_impl(&self, name: &str) -> Result<Workflow> {
        self.fetch_one(
            "SELECT * FROM workflows WHERE name = ?",
            vec![name.into()],
            decode::workflow,
        )
        .await
    }

    pub(crate) async fn get_workflow_by_id_impl(&self, workflow_id: Uuid) -> Result<Workflow> {
        self.fetch_one(
            "SELECT * FROM workflows WHERE id = ?",
            vec![uuid_param(workflow_id)],
            decode::workflow,
        )
        .await
    }

    pub(crate) async fn list_workflows_impl(&self) -> Result<Vec<Workflow>> {
        self.fetch_all(
            "SELECT * FROM workflows ORDER BY created_at DESC",
            Vec::new(),
            decode::workflow,
        )
        .await
    }

    pub(crate) async fn list_workflows_page_impl(
        &self,
        query: &WorkflowListQuery,
    ) -> Result<WorkflowListPage> {
        let search = query
            .search
            .as_ref()
            .map(|term| format!("%{}%", term.to_lowercase()));

        let (total, items) = match search {
            Some(pattern) => {
                let total = self
                    .scalar_i64(
                        "SELECT COUNT(*) AS total FROM workflows WHERE LOWER(name) LIKE ?",
                        vec![pattern.clone().into()],
                        "total",
                    )
                    .await?;
                let items = self
                    .fetch_all(
                        "SELECT * FROM workflows WHERE LOWER(name) LIKE ? ORDER BY name ASC LIMIT ? OFFSET ?",
                        vec![pattern.into(), query.limit.into(), query.offset.into()],
                        decode::workflow,
                    )
                    .await?;
                (total, items)
            }
            None => {
                let total = self
                    .scalar_i64(
                        "SELECT COUNT(*) AS total FROM workflows",
                        Vec::new(),
                        "total",
                    )
                    .await?;
                let items = self
                    .fetch_all(
                        "SELECT * FROM workflows ORDER BY name ASC LIMIT ? OFFSET ?",
                        vec![query.limit.into(), query.offset.into()],
                        decode::workflow,
                    )
                    .await?;
                (total, items)
            }
        };

        Ok(WorkflowListPage {
            items,
            total: total as usize,
        })
    }

    pub(crate) async fn delete_workflow_impl(&self, name: &str) -> Result<()> {
        self.execute("DELETE FROM workflows WHERE name = ?", vec![name.into()])
            .await
    }

    pub(crate) async fn get_workflows_by_ids_impl(
        &self,
        workflow_ids: &[Uuid],
    ) -> Result<Vec<Workflow>> {
        if workflow_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut params = Vec::with_capacity(workflow_ids.len());
        let placeholders = workflow_ids
            .iter()
            .map(|workflow_id| {
                params.push(uuid_param(*workflow_id));
                "?"
            })
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT * FROM workflows WHERE id IN ({placeholders})");
        self.fetch_all(&sql, params, decode::workflow).await
    }

    pub(crate) async fn create_workflow_tasks_impl(
        &self,
        workflow_id: Uuid,
        tasks: &[NewWorkflowTask],
    ) -> Result<()> {
        self.execute(
            "DELETE FROM workflow_tasks WHERE workflow_id = ?",
            vec![uuid_param(workflow_id)],
        )
        .await?;

        if tasks.is_empty() {
            return Ok(());
        }

        let mut params = Vec::with_capacity(tasks.len() * 8);
        let values = tasks
            .iter()
            .map(|task| {
                params.push(uuid_param(Uuid::new_v4()));
                params.push(uuid_param(workflow_id));
                params.push(task.task_index.into());
                params.push(task.task_name.clone().into());
                params.push(task.executor_type.clone().into());
                params.push(crate::database::json_text(&task.depends_on).into());
                params.push(crate::client::QueryParam::Text(task.params.to_string()));
                params.push(opt_json(task.signature.clone()));
                "(?, ?, ?, ?, ?, ?, ?, ?)"
            })
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "INSERT INTO workflow_tasks (id, workflow_id, task_index, task_name, executor_type, depends_on, params, signature) VALUES {values}"
        );
        self.execute(&sql, params).await
    }

    pub(crate) async fn update_workflow_task_params_impl(
        &self,
        workflow_id: Uuid,
        task_params: Option<Value>,
    ) -> Result<()> {
        self.execute(
            "UPDATE workflows SET task_params = ?, updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            vec![opt_json(task_params), uuid_param(workflow_id)],
        )
        .await
    }

    pub(crate) async fn list_workflow_tasks_impl(
        &self,
        workflow_id: Uuid,
    ) -> Result<Vec<WorkflowTask>> {
        self.fetch_all(
            "SELECT * FROM workflow_tasks WHERE workflow_id = ? ORDER BY task_index",
            vec![uuid_param(workflow_id)],
            decode::workflow_task,
        )
        .await
    }

    pub(crate) async fn create_or_get_snapshot_impl(
        &self,
        workflow_id: Uuid,
        content_hash: &str,
        tasks_json: Value,
        source_yaml: Option<String>,
    ) -> Result<WorkflowSnapshot> {
        self.fetch_one(
            r#"INSERT INTO workflow_snapshots (
                id, workflow_id, content_hash, tasks_json, source_yaml
            ) VALUES (?, ?, ?, ?, ?)
            ON CONFLICT (workflow_id, content_hash)
            DO UPDATE SET source_yaml = COALESCE(excluded.source_yaml, workflow_snapshots.source_yaml)
            RETURNING *"#,
            vec![
                uuid_param(Uuid::new_v4()),
                uuid_param(workflow_id),
                content_hash.into(),
                crate::client::QueryParam::Text(tasks_json.to_string()),
                opt_text_owned(source_yaml),
            ],
            decode::workflow_snapshot,
        )
        .await
    }

    pub(crate) async fn get_snapshot_impl(&self, snapshot_id: Uuid) -> Result<WorkflowSnapshot> {
        self.fetch_one(
            "SELECT * FROM workflow_snapshots WHERE id = ?",
            vec![uuid_param(snapshot_id)],
            decode::workflow_snapshot,
        )
        .await
    }

    pub(crate) async fn update_workflow_snapshot_impl(
        &self,
        workflow_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<()> {
        self.execute(
            "UPDATE workflows SET current_snapshot_id = ?, updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            vec![uuid_param(snapshot_id), uuid_param(workflow_id)],
        )
        .await
    }
}

#[async_trait]
impl WorkflowRepository for D1Database {
    async fn create_workflow(
        &self,
        name: &str,
        description: Option<&str>,
        job_name: &str,
        region: &str,
        project: &str,
        executor_type: &str,
        task_params: Option<Value>,
        schedule: Option<&str>,
    ) -> Result<Workflow> {
        self.create_workflow_impl(
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

    async fn get_workflow(&self, name: &str) -> Result<Workflow> {
        self.get_workflow_impl(name).await
    }

    async fn get_workflow_by_id(&self, workflow_id: Uuid) -> Result<Workflow> {
        self.get_workflow_by_id_impl(workflow_id).await
    }

    async fn list_workflows(&self) -> Result<Vec<Workflow>> {
        self.list_workflows_impl().await
    }

    async fn list_workflows_page(&self, query: &WorkflowListQuery) -> Result<WorkflowListPage> {
        self.list_workflows_page_impl(query).await
    }

    async fn delete_workflow(&self, name: &str) -> Result<()> {
        self.delete_workflow_impl(name).await
    }

    async fn get_workflows_by_ids(&self, workflow_ids: &[Uuid]) -> Result<Vec<Workflow>> {
        self.get_workflows_by_ids_impl(workflow_ids).await
    }

    async fn create_workflow_tasks(
        &self,
        workflow_id: Uuid,
        tasks: &[NewWorkflowTask],
    ) -> Result<()> {
        self.create_workflow_tasks_impl(workflow_id, tasks).await
    }

    async fn update_workflow_task_params(
        &self,
        workflow_id: Uuid,
        task_params: Option<Value>,
    ) -> Result<()> {
        self.update_workflow_task_params_impl(workflow_id, task_params)
            .await
    }

    async fn list_workflow_tasks(&self, workflow_id: Uuid) -> Result<Vec<WorkflowTask>> {
        self.list_workflow_tasks_impl(workflow_id).await
    }
}

#[async_trait]
impl WorkflowSnapshotRepository for D1Database {
    async fn create_or_get_snapshot(
        &self,
        workflow_id: Uuid,
        content_hash: &str,
        tasks_json: Value,
        source_yaml: Option<String>,
    ) -> Result<WorkflowSnapshot> {
        self.create_or_get_snapshot_impl(workflow_id, content_hash, tasks_json, source_yaml)
            .await
    }

    async fn get_snapshot(&self, snapshot_id: Uuid) -> Result<WorkflowSnapshot> {
        self.get_snapshot_impl(snapshot_id).await
    }

    async fn update_workflow_snapshot(&self, workflow_id: Uuid, snapshot_id: Uuid) -> Result<()> {
        self.update_workflow_snapshot_impl(workflow_id, snapshot_id)
            .await
    }
}
