use super::*;
use crate::compiled::{CompiledTask, CompiledWorkflow};
use crate::workflow::TaskDefinition;
use indexmap::IndexMap;
use std::path::PathBuf;

fn sample_workflow() -> Workflow {
    let mut tasks = IndexMap::new();
    tasks.insert(
        "task_a".to_string(),
        TaskDefinition {
            executor: ExecutorKind::Process,
            file: None,
            config_file: None,
            command: Some("echo ok".to_string()),
            retries: 1,
            ..Default::default()
        },
    );
    Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: std::collections::BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    }
}

#[test]
fn test_task_spec_from_workflow_and_missing_task() {
    let workflow = sample_workflow();
    let spec = TaskSpec::from_workflow(&workflow, "run-1".to_string(), "task_a", 2)
        .expect("task should exist");

    assert_eq!(spec.workflow_name, "wf");
    assert_eq!(spec.task_name, "task_a");
    assert_eq!(spec.attempt, 2);
    assert!(matches!(spec.executor, ExecutorKind::Process));
    assert!(TaskSpec::from_workflow(&workflow, "run-1".to_string(), "missing", 1).is_none());
}

#[test]
fn test_task_spec_from_compiled_and_missing_index() {
    let compiled = CompiledWorkflow {
        name: "wf-compiled".to_string(),
        vars: std::collections::BTreeMap::new(),
        types: std::collections::BTreeMap::new(),
        tasks: vec![CompiledTask {
            name: "task0".to_string(),
            executor: ExecutorKind::Process,
            command: Some("echo".to_string()),
            ..Default::default()
        }],
        name_index: IndexMap::from([(String::from("task0"), 0)]),
        topo: vec![0],
        root: PathBuf::from("."),
        hooks: vec![],
    };

    let spec = TaskSpec::from_compiled(&compiled, "run-2".to_string(), 0, 1)
        .expect("compiled task should exist");
    assert_eq!(spec.workflow_name, "wf-compiled");
    assert_eq!(spec.task_name, "task0");
    assert!(TaskSpec::from_compiled(&compiled, "run-2".to_string(), 9, 1).is_none());
}

#[test]
fn test_run_new_initializes_pending_state() {
    let workflow = sample_workflow();
    let run = Run::new(&workflow);
    assert_eq!(run.workflow, workflow.name);
    assert_eq!(run.status, RunStatus::Pending);
    assert!(run.started_at.is_none());
    assert!(run.finished_at.is_none());
    assert!(!run.id.is_empty());
}
