use std::path::{Path, PathBuf};

use crate::models::{Workflow, json_inner};

const REGISTRATION_KEY: &str = "registration";
const REGISTRATION_KIND_KEY: &str = "kind";
const REGISTRATION_KIND_DISCOVERED: &str = "discovered_workflow_definition";
const DEFINITIONS_ROOT_KEY: &str = "definitions_root";
const SOURCE_PATH_KEY: &str = "source_path";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRegistrationMetadata {
    pub definitions_root: PathBuf,
    pub source_path: PathBuf,
}

impl WorkflowRegistrationMetadata {
    pub fn new(definitions_root: &Path, source_path: &Path) -> Self {
        Self {
            definitions_root: canonicalize_or_clone(definitions_root),
            source_path: canonicalize_or_clone(source_path),
        }
    }
}

pub fn with_registration_metadata(
    task_params: Option<serde_json::Value>,
    registration: &WorkflowRegistrationMetadata,
) -> serde_json::Value {
    let mut params = task_params.unwrap_or_else(|| serde_json::json!({}));
    let Some(root) = params.as_object_mut() else {
        return serde_json::json!({
            REGISTRATION_KEY: registration_json(registration),
        });
    };
    root.insert(
        REGISTRATION_KEY.to_string(),
        registration_json(registration),
    );
    params
}

pub fn registration_from_workflow(workflow: &Workflow) -> Option<WorkflowRegistrationMetadata> {
    registration_from_params(workflow.task_params.as_ref().map(json_inner)?)
}

pub fn workflow_is_active_for_definitions_root(
    workflow: &Workflow,
    definitions_root: &Path,
) -> bool {
    match registration_from_workflow(workflow) {
        Some(registration) => {
            registration.definitions_root == canonicalize_or_clone(definitions_root)
                && registration.source_path.exists()
        }
        None => true,
    }
}

pub fn active_definitions_root_from_env() -> Option<PathBuf> {
    std::env::var("CZAR_WORKFLOW_DEFINITIONS_DIR")
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .map(PathBuf::from)
        .map(|path| canonicalize_or_clone(&path))
}

fn registration_from_params(params: &serde_json::Value) -> Option<WorkflowRegistrationMetadata> {
    let registration = params.get(REGISTRATION_KEY)?;
    let kind = registration.get(REGISTRATION_KIND_KEY)?.as_str()?;
    if kind != REGISTRATION_KIND_DISCOVERED {
        return None;
    }

    let definitions_root = PathBuf::from(registration.get(DEFINITIONS_ROOT_KEY)?.as_str()?);
    let source_path = PathBuf::from(registration.get(SOURCE_PATH_KEY)?.as_str()?);
    Some(WorkflowRegistrationMetadata {
        definitions_root,
        source_path,
    })
}

fn registration_json(registration: &WorkflowRegistrationMetadata) -> serde_json::Value {
    serde_json::json!({
        REGISTRATION_KIND_KEY: REGISTRATION_KIND_DISCOVERED,
        DEFINITIONS_ROOT_KEY: registration.definitions_root.display().to_string(),
        SOURCE_PATH_KEY: registration.source_path.display().to_string(),
    })
}

fn canonicalize_or_clone(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Workflow, json_value};
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn registration_metadata_round_trips() {
        let registration = WorkflowRegistrationMetadata::new(
            Path::new("/tmp/project"),
            Path::new("/tmp/project/workflow.yaml"),
        );
        let params = with_registration_metadata(
            Some(serde_json::json!({"workflow_format": "v2"})),
            &registration,
        );

        let workflow = Workflow {
            id: Uuid::new_v4(),
            name: "demo".to_string(),
            description: None,
            job_name: "dag".to_string(),
            region: "local".to_string(),
            project: "local".to_string(),
            executor_type: "dag".to_string(),
            task_params: Some(json_value(params)),
            schedule: None,
            schedule_enabled: false,
            last_scheduled_at: None,
            next_scheduled_at: None,
            current_snapshot_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(registration_from_workflow(&workflow), Some(registration));
    }

    #[test]
    fn workflows_without_registration_stay_visible() {
        let workflow = Workflow {
            id: Uuid::new_v4(),
            name: "manual".to_string(),
            description: None,
            job_name: "dag".to_string(),
            region: "local".to_string(),
            project: "local".to_string(),
            executor_type: "dag".to_string(),
            task_params: Some(json_value(serde_json::json!({"workflow_format": "v2"}))),
            schedule: None,
            schedule_enabled: false,
            last_scheduled_at: None,
            next_scheduled_at: None,
            current_snapshot_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(workflow_is_active_for_definitions_root(
            &workflow,
            Path::new("/tmp/project"),
        ));
    }
}
