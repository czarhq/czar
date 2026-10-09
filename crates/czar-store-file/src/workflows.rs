use anyhow::Result;
use chrono::Utc;
use tokio::fs;
use uuid::Uuid;

use czar_core::database::{NewWorkflowTask, WorkflowListPage, WorkflowListQuery};
use czar_core::models::{Workflow, WorkflowSnapshot, WorkflowTask};

use super::core::FileDatabase;

impl FileDatabase {
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
        self.ensure_dirs().await?;

        let workflow = Workflow {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            job_name: job_name.to_string(),
            region: region.to_string(),
            project: project.to_string(),
            executor_type: executor_type.to_string(),
            task_params: task_params.map(Into::into),
            schedule: schedule.map(|s| s.to_string()),
            schedule_enabled: false,
            last_scheduled_at: None,
            next_scheduled_at: None,
            current_snapshot_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.write_json(&self.workflow_path(workflow.id), &workflow)
            .await?;
        Ok(workflow)
    }

    pub(super) async fn get_workflow_impl(&self, name: &str) -> Result<Workflow> {
        let mut entries = fs::read_dir(self.workflows_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(workflow) = self.read_json::<Workflow>(&entry.path()).await
                && workflow.name == name
            {
                return Ok(workflow);
            }
        }
        anyhow::bail!("Workflow not found: {}", name)
    }

    pub(super) async fn list_workflows_impl(&self) -> Result<Vec<Workflow>> {
        self.ensure_dirs().await?;
        let mut workflows = Vec::new();
        let mut entries = fs::read_dir(self.workflows_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(workflow) = self.read_json::<Workflow>(&entry.path()).await {
                workflows.push(workflow);
            }
        }
        workflows.sort_by_key(|workflow| std::cmp::Reverse(workflow.created_at));
        Ok(workflows)
    }

    pub(super) async fn list_workflows_page_impl(
        &self,
        query: &WorkflowListQuery,
    ) -> Result<WorkflowListPage> {
        let mut workflows = self.list_workflows_impl().await?;
        workflows.sort_by(|a, b| a.name.cmp(&b.name));

        if let Some(search_term) = &query.search {
            let search_term = search_term.to_lowercase();
            workflows.retain(|wf| wf.name.to_lowercase().contains(&search_term));
        }

        let total = workflows.len();
        let items = workflows
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .collect();

        Ok(WorkflowListPage { items, total })
    }

    pub(super) async fn delete_workflow_impl(&self, name: &str) -> Result<()> {
        self.ensure_dirs().await?;
        let workflow = self.get_workflow_impl(name).await?;
        let runs = self.list_runs_impl(Some(workflow.id)).await?;
        for run in runs {
            let tasks = self.list_tasks_impl(run.id).await?;
            for task in tasks {
                let _ = fs::remove_file(self.task_path(task.id)).await;
            }
            let _ = fs::remove_file(self.run_path(run.id)).await;
        }
        let _ = fs::remove_file(self.workflow_tasks_path(workflow.id)).await;
        let _ = fs::remove_file(self.workflow_task_states_path(workflow.id)).await;
        let _ = fs::remove_file(self.workflow_path(workflow.id)).await;
        Ok(())
    }

    pub(super) async fn get_workflows_by_ids_impl(
        &self,
        workflow_ids: &[Uuid],
    ) -> Result<Vec<Workflow>> {
        let mut workflows = Vec::new();
        for id in workflow_ids {
            if let Ok(workflow) = self.read_json::<Workflow>(&self.workflow_path(*id)).await {
                workflows.push(workflow);
            }
        }
        Ok(workflows)
    }

    pub(super) async fn get_workflow_by_id_impl(&self, workflow_id: Uuid) -> Result<Workflow> {
        self.read_json(&self.workflow_path(workflow_id)).await
    }

    pub(super) async fn create_workflow_tasks_impl(
        &self,
        workflow_id: Uuid,
        tasks: &[NewWorkflowTask],
    ) -> Result<()> {
        self.ensure_dirs().await?;
        let now = Utc::now();
        let mut workflow_tasks = Vec::with_capacity(tasks.len());
        for task in tasks {
            workflow_tasks.push(WorkflowTask {
                id: Uuid::new_v4(),
                workflow_id,
                task_index: task.task_index,
                task_name: task.task_name.clone(),
                executor_type: task.executor_type.clone(),
                depends_on: task.depends_on.clone(),
                params: Some(task.params.clone().into()),
                signature: task.signature.clone().map(Into::into),
                created_at: now,
            });
        }
        self.write_json(&self.workflow_tasks_path(workflow_id), &workflow_tasks)
            .await?;
        Ok(())
    }

    pub(super) async fn update_workflow_task_params_impl(
        &self,
        workflow_id: Uuid,
        task_params: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut workflow = self.get_workflow_by_id_impl(workflow_id).await?;
        workflow.task_params = task_params.map(Into::into);
        workflow.updated_at = Utc::now();
        self.write_json(&self.workflow_path(workflow_id), &workflow)
            .await?;
        Ok(())
    }

    pub(super) async fn list_workflow_tasks_impl(
        &self,
        workflow_id: Uuid,
    ) -> Result<Vec<WorkflowTask>> {
        let path = self.workflow_tasks_path(workflow_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut tasks: Vec<WorkflowTask> = self.read_json(&path).await?;
        tasks.sort_by_key(|task| task.task_index);
        Ok(tasks)
    }

    pub(super) async fn get_due_scheduled_workflows_impl(&self) -> Result<Vec<Workflow>> {
        // File database doesn't support live scheduling
        Ok(Vec::new())
    }

    pub(super) async fn update_workflow_schedule_times_impl(
        &self,
        _workflow_id: Uuid,
        _last_scheduled_at: chrono::DateTime<chrono::Utc>,
        _next_scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        // File database doesn't support live scheduling
        Ok(())
    }

    pub(super) async fn update_workflow_schedule_impl(
        &self,
        _workflow_id: Uuid,
        _schedule: Option<&str>,
        _schedule_enabled: bool,
    ) -> Result<()> {
        // File database doesn't support live scheduling
        Ok(())
    }

    pub(super) async fn create_or_get_snapshot_impl(
        &self,
        workflow_id: Uuid,
        content_hash: &str,
        tasks_json: serde_json::Value,
        source_yaml: Option<String>,
    ) -> Result<WorkflowSnapshot> {
        self.ensure_dirs().await?;

        // Check for existing snapshot with same hash
        let snapshots_dir = self.base.join("snapshots");
        fs::create_dir_all(&snapshots_dir).await?;

        let mut entries = fs::read_dir(&snapshots_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let snapshot = self.read_json::<WorkflowSnapshot>(&entry.path()).await?;
            if snapshot.workflow_id == workflow_id && snapshot.content_hash == content_hash {
                return Ok(snapshot);
            }
        }

        // Create new snapshot
        let snapshot = WorkflowSnapshot {
            id: Uuid::new_v4(),
            workflow_id,
            content_hash: content_hash.to_string(),
            tasks_json: tasks_json.into(),
            source_yaml,
            created_at: Utc::now(),
        };

        let snapshot_path = snapshots_dir.join(format!("{}.json", snapshot.id));
        self.write_json(&snapshot_path, &snapshot).await?;
        Ok(snapshot)
    }

    pub(super) async fn get_snapshot_impl(&self, snapshot_id: Uuid) -> Result<WorkflowSnapshot> {
        let snapshots_dir = self.base.join("snapshots");
        let snapshot_path = snapshots_dir.join(format!("{}.json", snapshot_id));
        self.read_json(&snapshot_path).await
    }

    pub(super) async fn update_workflow_snapshot_impl(
        &self,
        workflow_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<()> {
        let mut workflow = self.get_workflow_by_id_impl(workflow_id).await?;
        workflow.current_snapshot_id = Some(snapshot_id);
        workflow.updated_at = Utc::now();
        self.write_json(&self.workflow_path(workflow_id), &workflow)
            .await?;
        Ok(())
    }
}
