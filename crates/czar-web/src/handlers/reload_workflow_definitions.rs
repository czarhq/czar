use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::api::ApiServer;
use czar_core::database::Database;
use czar_core::services::workflow_yaml::{
    ApplyWorkflowYamlOutcome, ApplyWorkflowYamlRequest, ExistingWorkflowBehavior,
    apply_workflow_yaml,
};
use czar_core::workflow_registry::WorkflowRegistrationMetadata;
use czar_core::workflow_source::load_workflow_source;

#[derive(Default, Serialize)]
pub struct ReloadWorkflowDefinitionResult {
    pub file: String,
    pub workflow: Option<String>,
    pub outcome: String,
    pub message: Option<String>,
}

#[derive(Default, Serialize)]
pub struct ReloadWorkflowDefinitionsResponse {
    pub definitions_root: String,
    pub discovered: usize,
    pub reloaded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub results: Vec<ReloadWorkflowDefinitionResult>,
}

#[derive(Default, Serialize)]
pub struct WorkflowDefinitionsConfigResponse {
    pub definitions_root: String,
}

#[derive(Debug)]
pub enum ReloadWorkflowDefinitionsError {
    InvalidDefinitionsRoot(String),
    DiscoverDefinitions {
        definitions_root: std::path::PathBuf,
        source: std::io::Error,
    },
}

impl ReloadWorkflowDefinitionsError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidDefinitionsRoot(_) => StatusCode::BAD_REQUEST,
            Self::DiscoverDefinitions { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn into_response_parts(self) -> (StatusCode, String) {
        let status = self.status_code();
        let message = self.to_string();
        (status, message)
    }
}

impl std::fmt::Display for ReloadWorkflowDefinitionsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDefinitionsRoot(message) => f.write_str(message),
            Self::DiscoverDefinitions {
                definitions_root,
                source,
            } => write!(
                f,
                "Failed to discover workflow definitions in {}: {}",
                definitions_root.display(),
                source
            ),
        }
    }
}

impl std::error::Error for ReloadWorkflowDefinitionsError {}

fn collect_workflow_definition_sources(
    definitions_root: &std::path::Path,
) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut stack = vec![definitions_root.to_path_buf()];
    let mut files = Vec::new();

    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if should_ignore_workflow_support_dir(&path) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            let is_yaml = matches!(
                path.extension().and_then(|v| v.to_str()),
                Some("yaml" | "yml")
            );
            if is_yaml && !should_ignore_workflow_support_file(&path) {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(files)
}

fn should_ignore_workflow_support_dir(path: &std::path::Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(name, "docs" | "profiles" | "workflow-schema-reference")
}

fn should_ignore_workflow_support_file(path: &std::path::Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    if name.starts_with('.') {
        return true;
    }
    matches!(
        name,
        "czar.yaml" | "czar.yml" | "dbt_project.yml" | "profiles.yml"
    )
}

pub fn resolve_definitions_root(cli_override: Option<&str>) -> Result<std::path::PathBuf, String> {
    if let Some(raw) = cli_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| std::env::var("CZAR_WORKFLOW_DEFINITIONS_DIR").ok())
    {
        let path = std::path::PathBuf::from(raw);
        let path = path.canonicalize().unwrap_or(path);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!(
            "CZAR_WORKFLOW_DEFINITIONS_DIR does not exist: {}",
            path.display()
        ));
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    Ok(cwd.canonicalize().unwrap_or(cwd))
}

enum WorkflowSourceClassification {
    WorkflowDefinition,
    Snippet { reason: String },
}

fn is_placeholder_string(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with('<') && trimmed.ends_with('>')
}

fn classify_workflow_source(
    yaml_content: &str,
) -> Result<WorkflowSourceClassification, serde_yaml::Error> {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml_content)?;
    let Some(root) = value.as_mapping() else {
        return Ok(WorkflowSourceClassification::Snippet {
            reason: "top-level YAML is not a mapping".to_string(),
        });
    };

    let Some(name_value) = root.get(serde_yaml::Value::String("name".to_string())) else {
        return Ok(WorkflowSourceClassification::Snippet {
            reason: "missing top-level 'name'".to_string(),
        });
    };
    let Some(tasks_value) = root.get(serde_yaml::Value::String("tasks".to_string())) else {
        return Ok(WorkflowSourceClassification::Snippet {
            reason: "missing top-level 'tasks'".to_string(),
        });
    };

    let Some(name) = name_value.as_str() else {
        return Ok(WorkflowSourceClassification::Snippet {
            reason: "top-level 'name' is not a concrete string".to_string(),
        });
    };
    if is_placeholder_string(name) {
        return Ok(WorkflowSourceClassification::Snippet {
            reason: "top-level 'name' is a placeholder template".to_string(),
        });
    }

    let Some(tasks) = tasks_value.as_mapping() else {
        return Ok(WorkflowSourceClassification::Snippet {
            reason: "top-level 'tasks' is not a mapping".to_string(),
        });
    };

    for (task_name_value, task_value) in tasks {
        let Some(task_name) = task_name_value.as_str() else {
            return Ok(WorkflowSourceClassification::Snippet {
                reason: "task names are not concrete strings".to_string(),
            });
        };
        if is_placeholder_string(task_name) {
            return Ok(WorkflowSourceClassification::Snippet {
                reason: format!("task '{task_name}' is a placeholder template"),
            });
        }

        let Some(task_mapping) = task_value.as_mapping() else {
            continue;
        };
        if let Some(executor) = task_mapping
            .get(serde_yaml::Value::String("executor".to_string()))
            .and_then(serde_yaml::Value::as_str)
            && (is_placeholder_string(executor) || executor.contains('|'))
        {
            return Ok(WorkflowSourceClassification::Snippet {
                reason: format!(
                    "task '{task_name}' executor '{executor}' is a placeholder/template value"
                ),
            });
        }
    }

    Ok(WorkflowSourceClassification::WorkflowDefinition)
}

pub async fn reload_workflow_definitions_for_db(
    db: &dyn Database,
    definitions_root: &std::path::Path,
) -> Result<ReloadWorkflowDefinitionsResponse, ReloadWorkflowDefinitionsError> {
    let definitions_root = definitions_root
        .canonicalize()
        .unwrap_or_else(|_| definitions_root.to_path_buf());

    let files = collect_workflow_definition_sources(&definitions_root).map_err(|source| {
        ReloadWorkflowDefinitionsError::DiscoverDefinitions {
            definitions_root: definitions_root.clone(),
            source,
        }
    })?;

    let mut reloaded = 0usize;
    let skipped = 0usize;
    let mut failed = 0usize;
    let mut results = Vec::with_capacity(files.len());

    for file in files {
        let yaml_content = match load_workflow_source(&file) {
            Ok(content) => content,
            Err(err) => {
                failed += 1;
                results.push(ReloadWorkflowDefinitionResult {
                    file: file.display().to_string(),
                    workflow: None,
                    outcome: "failed".to_string(),
                    message: Some(format!("Failed to load workflow source: {}", err)),
                });
                continue;
            }
        };

        match classify_workflow_source(&yaml_content) {
            Ok(WorkflowSourceClassification::WorkflowDefinition) => {}
            Ok(WorkflowSourceClassification::Snippet { reason }) => {
                let _ = reason;
                continue;
            }
            Err(err) => {
                failed += 1;
                results.push(ReloadWorkflowDefinitionResult {
                    file: file.display().to_string(),
                    workflow: None,
                    outcome: "failed".to_string(),
                    message: Some(format!("Failed to inspect YAML structure: {}", err)),
                });
                continue;
            }
        }

        let root = file
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| definitions_root.clone());
        let root = root.canonicalize().unwrap_or(root);
        let registration = WorkflowRegistrationMetadata::new(&definitions_root, &file);

        match apply_workflow_yaml(
            db,
            ApplyWorkflowYamlRequest::new(&yaml_content, &root, "local", "local")
                .with_existing_workflow(ExistingWorkflowBehavior::Replace)
                .with_persist_schedule_on_create(true)
                .with_registration_metadata(registration),
        )
        .await
        {
            Ok(result) => {
                reloaded += 1;
                let outcome = match result.outcome {
                    ApplyWorkflowYamlOutcome::Created => "created",
                    ApplyWorkflowYamlOutcome::Replaced => "replaced",
                };
                results.push(ReloadWorkflowDefinitionResult {
                    file: file.display().to_string(),
                    workflow: Some(result.workflow.name),
                    outcome: outcome.to_string(),
                    message: None,
                });
            }
            Err(err) => {
                failed += 1;
                results.push(ReloadWorkflowDefinitionResult {
                    file: file.display().to_string(),
                    workflow: None,
                    outcome: "failed".to_string(),
                    message: Some(err.to_string()),
                });
            }
        }
    }

    Ok(ReloadWorkflowDefinitionsResponse {
        definitions_root: definitions_root.display().to_string(),
        discovered: results.len(),
        reloaded,
        skipped,
        failed,
        results,
    })
}

pub async fn workflow_definitions_root(State(api): State<ApiServer>) -> impl IntoResponse {
    Json(WorkflowDefinitionsConfigResponse {
        definitions_root: api.workflow_definitions_root().display().to_string(),
    })
}

pub async fn reload_workflow_definitions(State(api): State<ApiServer>) -> impl IntoResponse {
    match reload_workflow_definitions_for_db(&*api.db, api.workflow_definitions_root()).await {
        Ok(response) => Json(response).into_response(),
        Err(err) => {
            let (status, message) = err.into_response_parts();
            (status, message).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_definitions_root_defaults_to_current_dir() {
        let cwd = std::env::current_dir().expect("cwd");
        let resolved = resolve_definitions_root(None).expect("resolve");
        assert_eq!(resolved, cwd.canonicalize().unwrap_or(cwd));
    }

    #[test]
    fn classify_skips_yaml_without_workflow_fields() {
        let yaml = "extract:\n  executor: python\n";
        let classification = classify_workflow_source(yaml).expect("classify");
        assert!(matches!(
            classification,
            WorkflowSourceClassification::Snippet { .. }
        ));
    }

    #[test]
    fn classify_skips_placeholder_schema_templates() {
        let yaml = "name: <workflow_name>\ntasks:\n  <task_name>:\n    executor: python|process\n";
        let classification = classify_workflow_source(yaml).expect("classify");
        assert!(matches!(
            classification,
            WorkflowSourceClassification::Snippet { .. }
        ));
    }

    #[test]
    fn classify_keeps_concrete_workflows() {
        let yaml = "name: demo\ntasks:\n  extract:\n    executor: python\n    module: demo\n";
        let classification = classify_workflow_source(yaml).expect("classify");
        assert!(matches!(
            classification,
            WorkflowSourceClassification::WorkflowDefinition
        ));
    }

    #[test]
    fn collect_workflow_definition_sources_ignores_support_yaml() {
        let root = std::env::temp_dir().join(format!("czar-reload-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("demo")).expect("create demo dir");
        std::fs::create_dir_all(root.join("polyglot/docs")).expect("create docs dir");
        std::fs::create_dir_all(root.join("dbt/profiles")).expect("create profiles dir");
        std::fs::create_dir_all(root.join("workflow-schema-reference")).expect("create schema dir");

        std::fs::write(root.join("demo/workflow.yaml"), "name: demo\ntasks: {}\n")
            .expect("write workflow");
        std::fs::write(
            root.join("polyglot/docs/library-executor.yaml"),
            "name: snippet\n",
        )
        .expect("write docs yaml");
        std::fs::write(root.join("dbt/profiles/profiles.yml"), "default: {}\n")
            .expect("write profiles yaml");
        std::fs::write(
            root.join("workflow-schema-reference/valid-example.yaml"),
            "name: example\ntasks: {}\n",
        )
        .expect("write schema example");
        std::fs::write(root.join("demo/czar.yaml"), "env_file: .env\n").expect("write czar config");

        let collected = collect_workflow_definition_sources(&root).expect("collect");
        let labels = collected
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .expect("relative path")
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(labels, vec!["demo/workflow.yaml"]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
