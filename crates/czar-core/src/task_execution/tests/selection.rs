use super::*;
use crate::database::NewTask;
use crate::models::{RunTaskSelection, TaskSelectionMode, WorkflowTask, json_value};
use chrono::Utc;

#[test]
fn test_build_run_tasks_normalizes_params_and_clamps_limits() {
    let run_id = Uuid::new_v4();
    let workflow_id = Uuid::new_v4();
    let workflow = sample_workflow(workflow_id);
    let now = Utc::now();
    let workflow_tasks = vec![
        WorkflowTask {
            id: Uuid::new_v4(),
            workflow_id,
            task_index: 0,
            task_name: "a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: Some(json_value(serde_json::json!("bad"))),
            signature: None,
            created_at: now,
        },
        WorkflowTask {
            id: Uuid::new_v4(),
            workflow_id,
            task_index: 1,
            task_name: "b".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["a".to_string()],
            params: Some(json_value(serde_json::json!({
                "max_retries": i64::MAX,
                "timeout_seconds": -20,
                "task_name": "custom-name"
            }))),
            signature: None,
            created_at: now,
        },
    ];

    let built = build_run_tasks(run_id, &workflow, &workflow_tasks, None);
    assert_eq!(built.len(), 2);

    assert_eq!(built[0].max_retries, 0);
    assert_eq!(built[0].timeout_seconds, None);
    assert_eq!(built[0].params["task_index"], 0);
    assert_eq!(built[0].params["task_name"], "a");
    assert_eq!(built[0].params["workflow_name"], "wf");
    assert_eq!(built[0].params["run_id"], run_id.to_string());

    assert_eq!(built[1].max_retries, i32::MAX);
    assert_eq!(built[1].timeout_seconds, Some(0));
    assert_eq!(built[1].params["task_name"], "custom-name");
}

#[test]
fn test_build_run_tasks_merges_run_inputs_into_workflow_vars() {
    let run_id = Uuid::new_v4();
    let workflow_id = Uuid::new_v4();
    let workflow = sample_workflow(workflow_id);
    let now = Utc::now();
    let workflow_tasks = vec![WorkflowTask {
        id: Uuid::new_v4(),
        workflow_id,
        task_index: 0,
        task_name: "a".to_string(),
        executor_type: "process".to_string(),
        depends_on: vec![],
        params: Some(json_value(serde_json::json!({
            "workflow_vars": {
                "PROFILE": "debug",
                "shared": "workflow"
            }
        }))),
        signature: None,
        created_at: now,
    }];
    let run_inputs = serde_json::json!({
        "date": "2024-01-15",
        "shared": "run"
    });

    let built = build_run_tasks(run_id, &workflow, &workflow_tasks, run_inputs.as_object());

    assert_eq!(built[0].params["workflow_vars"]["PROFILE"], "debug");
    assert_eq!(built[0].params["workflow_vars"]["date"], "2024-01-15");
    assert_eq!(built[0].params["workflow_vars"]["shared"], "run");
}

#[test]
fn test_resolve_task_selection_full_expands_upstream_and_downstream() {
    let graph = vec![
        ("a".to_string(), vec![]),
        ("b".to_string(), vec!["a".to_string()]),
        ("c".to_string(), vec!["b".to_string()]),
        ("d".to_string(), vec!["a".to_string()]),
    ];

    let resolved = resolve_task_selection(&graph, &["b".to_string()], TaskSelectionMode::Full)
        .expect("selection should resolve");
    assert_eq!(resolved, vec!["a", "b", "c"]);
}

#[test]
fn test_resolve_task_selection_exact_allows_missing_dependencies() {
    let graph = vec![
        ("a".to_string(), vec![]),
        ("b".to_string(), vec!["a".to_string()]),
    ];

    let resolved = resolve_task_selection(&graph, &["b".to_string()], TaskSelectionMode::Exact)
        .expect("exact selection should allow selecting a task without its dependencies");
    assert_eq!(resolved, vec!["b"]);
}

#[test]
fn test_filter_run_tasks_by_selection_allows_missing_dependencies() {
    let tasks = vec![
        NewTask {
            task_index: 0,
            task_name: "a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({}),
            max_retries: 0,
            timeout_seconds: None,
        },
        NewTask {
            task_index: 1,
            task_name: "b".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["a".to_string()],
            params: serde_json::json!({}),
            max_retries: 0,
            timeout_seconds: None,
        },
    ];

    let selection = RunTaskSelection {
        mode: TaskSelectionMode::Exact,
        flow_name: None,
        requested_tasks: vec!["b".to_string()],
        requested_groups: vec![],
        resolved_tasks: vec!["b".to_string()],
    };

    let filtered = filter_run_tasks_by_selection(tasks, &selection)
        .expect("missing dependencies should be allowed for selected tasks");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].task_name, "b");
}
