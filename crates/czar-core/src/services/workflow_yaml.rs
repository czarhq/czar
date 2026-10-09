use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::compiled::build_workflow_tasks;
use crate::database::Database;
use crate::models::Workflow as StoredWorkflow;
use crate::workflow::{ValidationOptions, WorkflowDefinition};
use crate::workflow_registry::{WorkflowRegistrationMetadata, with_registration_metadata};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExistingWorkflowBehavior {
    Error,
    #[default]
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApplyWorkflowYamlOutcome {
    #[default]
    Created,
    Replaced,
}

#[derive(Debug, Clone, Default)]
pub struct ApplyWorkflowYamlResult {
    pub workflow: StoredWorkflow,
    pub outcome: ApplyWorkflowYamlOutcome,
}

pub struct ApplyWorkflowYamlRequest<'a> {
    pub yaml_content: &'a str,
    pub root: &'a Path,
    pub project: &'a str,
    pub region: &'a str,
    pub existing_workflow: ExistingWorkflowBehavior,
    pub persist_schedule_on_create: bool,
    pub var_overrides: BTreeMap<String, String>,
    pub validation_options: ValidationOptions,
    pub registration_metadata: Option<WorkflowRegistrationMetadata>,
}

impl Default for ApplyWorkflowYamlRequest<'static> {
    fn default() -> Self {
        Self {
            yaml_content: "",
            root: Path::new("."),
            project: "",
            region: "",
            existing_workflow: ExistingWorkflowBehavior::default(),
            persist_schedule_on_create: false,
            var_overrides: BTreeMap::new(),
            validation_options: ValidationOptions::STRICT,
            registration_metadata: None,
        }
    }
}

impl<'a> ApplyWorkflowYamlRequest<'a> {
    pub fn new(yaml_content: &'a str, root: &'a Path, project: &'a str, region: &'a str) -> Self {
        Self {
            yaml_content,
            root,
            project,
            region,
            existing_workflow: ExistingWorkflowBehavior::Replace,
            persist_schedule_on_create: false,
            var_overrides: BTreeMap::new(),
            validation_options: ValidationOptions::STRICT,
            registration_metadata: None,
        }
    }

    pub fn with_existing_workflow(mut self, behavior: ExistingWorkflowBehavior) -> Self {
        self.existing_workflow = behavior;
        self
    }

    pub fn with_persist_schedule_on_create(mut self, persist: bool) -> Self {
        self.persist_schedule_on_create = persist;
        self
    }

    pub fn with_var_overrides(mut self, var_overrides: BTreeMap<String, String>) -> Self {
        self.var_overrides = var_overrides;
        self
    }

    pub fn with_validation_options(mut self, validation_options: ValidationOptions) -> Self {
        self.validation_options = validation_options;
        self
    }

    pub fn with_registration_metadata(
        mut self,
        registration_metadata: WorkflowRegistrationMetadata,
    ) -> Self {
        self.registration_metadata = Some(registration_metadata);
        self
    }
}

#[derive(Debug, Error)]
pub enum WorkflowServiceError {
    #[error("workflow already exists: {name}")]
    WorkflowAlreadyExists { name: String },

    #[error("invalid workflow yaml: {0}")]
    InvalidYaml(#[from] serde_yaml::Error),

    #[error("{0}")]
    InvalidWorkflow(String),

    #[error("{0}")]
    Compilation(String),

    #[error(transparent)]
    Storage(#[from] anyhow::Error),
}

pub async fn apply_workflow_yaml(
    db: &dyn Database,
    request: ApplyWorkflowYamlRequest<'_>,
) -> Result<ApplyWorkflowYamlResult, WorkflowServiceError> {
    let definition = WorkflowDefinition::parse_yaml_with_root(request.yaml_content, request.root)
        .map_err(|err| WorkflowServiceError::InvalidWorkflow(err.to_string()))?;
    definition
        .validate_catalog_with_options(request.validation_options)
        .map_err(|err| WorkflowServiceError::InvalidWorkflow(err.to_string()))?;
    let flow_workflow = definition
        .resolve_flow_with_options(None, request.validation_options)
        .map_err(|err| WorkflowServiceError::InvalidWorkflow(err.to_string()))?;

    let compiled = flow_workflow
        .compile_with_options(
            request.root,
            &request.var_overrides,
            request.validation_options,
        )
        .map_err(|err| WorkflowServiceError::Compilation(err.to_string()))?;

    let existing = match db.get_workflow(&definition.name).await {
        Ok(workflow) => Some(workflow),
        Err(err) if is_not_found_error(&err) => None,
        Err(err) => return Err(WorkflowServiceError::Storage(err)),
    };

    let task_params = {
        let base = serde_json::json!({
            "workflow_format": "v2",
            "flows": definition.flow_names(),
        });
        request
            .registration_metadata
            .as_ref()
            .map(|registration| with_registration_metadata(Some(base.clone()), registration))
            .unwrap_or(base)
    };

    let (workflow, outcome) = match existing {
        Some(workflow) => match request.existing_workflow {
            ExistingWorkflowBehavior::Replace => (workflow, ApplyWorkflowYamlOutcome::Replaced),
            ExistingWorkflowBehavior::Error => {
                return Err(WorkflowServiceError::WorkflowAlreadyExists {
                    name: definition.name.clone(),
                });
            }
        },
        None => {
            let schedule = if request.persist_schedule_on_create {
                definition.schedule.as_deref()
            } else {
                None
            };
            let workflow = db
                .create_workflow(
                    &definition.name,
                    None,
                    "dag",
                    request.region,
                    request.project,
                    "dag",
                    Some(task_params.clone()),
                    schedule,
                )
                .await?;
            (workflow, ApplyWorkflowYamlOutcome::Created)
        }
    };

    db.update_workflow_task_params(workflow.id, Some(task_params))
        .await?;

    let workflow_tasks = build_workflow_tasks(&compiled);
    db.create_workflow_tasks(workflow.id, &workflow_tasks)
        .await?;

    // Create or reuse snapshot based on content hash
    let tasks_json = serde_json::to_value(&workflow_tasks)
        .map_err(|e| anyhow::anyhow!("Failed to serialize workflow tasks: {}", e))?;
    let content_hash = compute_content_hash(&tasks_json);

    let snapshot = db
        .create_or_get_snapshot(
            workflow.id,
            &content_hash,
            tasks_json,
            Some(
                serde_yaml::to_string(&definition)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize expanded workflow: {}", e))?,
            ),
        )
        .await?;

    // Update workflow's current_snapshot_id
    db.update_workflow_snapshot(workflow.id, snapshot.id)
        .await?;

    // Fetch the updated workflow to return
    let workflow = db.get_workflow_by_id(workflow.id).await?;

    Ok(ApplyWorkflowYamlResult { workflow, outcome })
}

fn compute_content_hash(tasks_json: &serde_json::Value) -> String {
    let canonical = serde_json::to_string(tasks_json).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

fn is_not_found_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("row not found") || msg.contains("no rows") || msg.contains("not found")
}
