use super::*;
use crate::error::WorkflowValidationError;
use std::path::PathBuf;
use tempfile::NamedTempFile;

mod parallelism;
mod types;

fn process_task() -> TaskDefinition {
    TaskDefinition {
        executor: ExecutorKind::Process,
        command: Some("echo ok".to_string()),
        ..Default::default()
    }
}

fn workflow_with_tasks(tasks: IndexMap<String, TaskDefinition>) -> Workflow {
    Workflow {
        name: "wf".to_string(),
        tasks,
        ..Default::default()
    }
}

#[test]
fn test_validate_rejects_empty_workflow_name_and_no_tasks() {
    let empty_name = Workflow {
        name: "  ".to_string(),
        ..Default::default()
    };
    assert_eq!(
        empty_name.validate(),
        Err(WorkflowValidationError::EmptyWorkflowName)
    );

    let no_tasks = Workflow {
        name: "wf".to_string(),
        ..Default::default()
    };
    assert_eq!(no_tasks.validate(), Err(WorkflowValidationError::NoTasks));
}

#[test]
fn test_validate_rejects_dependency_errors() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.depends_on = vec!["missing".to_string()];
    tasks.insert("a".to_string(), a);

    let wf = workflow_with_tasks(tasks);
    assert!(matches!(
        wf.validate(),
        Err(WorkflowValidationError::UnknownDependency { .. })
    ));

    let mut tasks = IndexMap::new();
    let mut self_dep = process_task();
    self_dep.depends_on = vec!["a".to_string()];
    tasks.insert("a".to_string(), self_dep);
    let wf = workflow_with_tasks(tasks);
    assert!(matches!(
        wf.validate(),
        Err(WorkflowValidationError::SelfDependency { task }) if task == "a"
    ));
}

#[test]
fn test_validate_rejects_executor_specific_missing_fields() {
    let mut tasks = IndexMap::new();
    tasks.insert(
        "python".to_string(),
        TaskDefinition {
            executor: ExecutorKind::Python,
            ..Default::default()
        },
    );
    assert!(matches!(
        workflow_with_tasks(tasks).validate(),
        Err(WorkflowValidationError::MissingTaskFile { .. })
    ));

    let mut tasks = IndexMap::new();
    tasks.insert(
        "process".to_string(),
        TaskDefinition {
            executor: ExecutorKind::Process,
            ..Default::default()
        },
    );
    assert!(matches!(
        workflow_with_tasks(tasks).validate(),
        Err(WorkflowValidationError::MissingTaskCommand { .. })
    ));

    let mut tasks = IndexMap::new();
    tasks.insert(
        "cloudrun".to_string(),
        TaskDefinition {
            executor: ExecutorKind::CloudRun,
            job: None,
            ..Default::default()
        },
    );
    assert!(matches!(
        workflow_with_tasks(tasks).validate(),
        Err(WorkflowValidationError::MissingTaskJob { .. })
    ));

    let mut tasks = IndexMap::new();
    tasks.insert(
        "lib".to_string(),
        TaskDefinition {
            executor: ExecutorKind::Library,
            ..Default::default()
        },
    );
    assert!(matches!(
        workflow_with_tasks(tasks).validate(),
        Err(WorkflowValidationError::MissingTaskFile { .. })
    ));

    let mut tasks = IndexMap::new();
    tasks.insert(
        "wasm".to_string(),
        TaskDefinition {
            executor: ExecutorKind::Wasm,
            ..Default::default()
        },
    );
    assert!(matches!(
        workflow_with_tasks(tasks).validate(),
        Err(WorkflowValidationError::MissingTaskFile { .. })
    ));

    let mut tasks = IndexMap::new();
    tasks.insert(
        "container".to_string(),
        TaskDefinition {
            executor: ExecutorKind::Container,
            command: Some("sh".to_string()),
            ..Default::default()
        },
    );
    assert!(matches!(
        workflow_with_tasks(tasks).validate(),
        Err(WorkflowValidationError::MissingTaskImage { .. })
    ));
}

#[test]
fn test_validate_rejects_object_env_values() {
    let mut task = process_task();
    task.env = serde_json::json!({
        "API_KEY": {
            "secret": "projects/demo/secrets/api-key"
        }
    });

    let wf = workflow_with_tasks(IndexMap::from([("extract".to_string(), task)]));

    assert!(matches!(
        wf.validate(),
        Err(WorkflowValidationError::Custom(msg))
            if msg.contains("string, number, or boolean literal")
    ));
}

#[test]
fn test_validate_accepts_secret_file_mounts_for_process_tasks() {
    let mut task = process_task();
    task.secrets = vec![SecretFileMount {
        name: "service-account".to_string(),
        secret: "projects/demo/secrets/service-account".to_string(),
        path: PathBuf::from(".czar/secrets/service-account.json"),
    }];

    let wf = workflow_with_tasks(IndexMap::from([("extract".to_string(), task)]));

    assert_eq!(wf.validate(), Ok(()));
}

#[test]
fn test_validate_rejects_secret_file_mounts_for_cloudrun_tasks() {
    let mut task = process_task();
    task.executor = ExecutorKind::CloudRun;
    task.command = None;
    task.job = Some("demo-job".to_string());
    task.secrets = vec![SecretFileMount {
        name: "service-account".to_string(),
        secret: "projects/demo/secrets/service-account".to_string(),
        path: PathBuf::from(".czar/secrets/service-account.json"),
    }];

    let wf = workflow_with_tasks(IndexMap::from([("extract".to_string(), task)]));

    assert!(matches!(
        wf.validate(),
        Err(WorkflowValidationError::Custom(msg))
            if msg.contains("field 'secrets' is only supported for process, python, and container executors")
    ));
}

#[test]
fn test_validate_detects_cycle() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    let mut b = process_task();
    a.depends_on = vec!["b".to_string()];
    b.depends_on = vec!["a".to_string()];
    tasks.insert("a".to_string(), a);
    tasks.insert("b".to_string(), b);

    let wf = workflow_with_tasks(tasks);
    assert!(matches!(
        wf.validate(),
        Err(WorkflowValidationError::Cycle { .. })
    ));
}

#[test]
fn test_validate_rejects_non_scalar_env_values() {
    let mut tasks = IndexMap::new();
    let mut task = process_task();
    task.env = serde_json::json!({
        "GOOD": "value",
        "BAD": {"nested": true}
    });
    tasks.insert("a".to_string(), task);

    let wf = workflow_with_tasks(tasks);
    let err = wf.validate().expect_err("nested env value should fail");
    assert!(err.to_string().contains("env 'BAD'"));
}

#[test]
fn test_load_parses_valid_yaml() {
    let mut file = NamedTempFile::new().expect("tempfile should be created");
    let yaml = r#"
name: hello
tasks:
  first:
    executor: process
    command: "echo hello"
"#;
    std::io::Write::write_all(&mut file, yaml.as_bytes()).expect("yaml content should be written");

    let loaded = Workflow::load(file.path()).expect("workflow yaml should load");
    assert_eq!(loaded.name, "hello");
    assert!(loaded.tasks.contains_key("first"));
}
