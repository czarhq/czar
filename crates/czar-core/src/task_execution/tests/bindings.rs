use super::*;
use std::collections::HashSet;

#[tokio::test]
async fn test_execute_task_resolves_task_bindings_into_task_input() {
    let mut task = sample_task_with_workflow(Some(serde_json::json!({
        "task_bindings": {
            "from_const": {"const": 15},
            "from_ref": {"ref": "tasks.dep.output.x"}
        }
    })));
    task.depends_on = vec!["dep".to_string()];

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashMap::from([("dep".to_string(), serde_json::json!({"x": 7}))]),
    )]));
    let executor = Arc::new(RecordingExecutor::new(false));
    let manager = RecordingManager {
        executor: executor.clone(),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(
        task,
        workflow_map,
        outputs_by_run,
        Arc::new(HashMap::new()),
        Arc::new(HashMap::new()),
        Arc::new(HashMap::new()),
        &manager,
        tx,
    )
    .await;
    assert!(result.is_ok());

    let (_id, _job_name, params) = executor
        .captured
        .lock()
        .await
        .clone()
        .expect("captured execute params");
    let params = params.expect("params should be present");
    assert_eq!(params["task_input"]["from_const"], 15);
    assert_eq!(params["task_input"]["from_ref"], 7);
    assert_eq!(params.get("upstream"), None);
}

#[tokio::test]
async fn test_execute_task_resolves_missing_dependency_from_persisted_state() {
    let mut task = sample_task_with_workflow(Some(serde_json::json!({
        "task_bindings": {
            "from_ref": {"ref": "tasks.dep.output.x"}
        }
    })));
    task.depends_on = vec!["dep".to_string()];

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::new());
    let available_run_tasks_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashSet::from([task.task_name.clone()]),
    )]));
    let persisted_outputs_by_workflow = Arc::new(HashMap::from([(
        task.workflow_id,
        HashMap::from([("dep".to_string(), serde_json::json!({"x": 9}))]),
    )]));
    let executor = Arc::new(RecordingExecutor::new(false));
    let manager = RecordingManager {
        executor: executor.clone(),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(
        task,
        workflow_map,
        outputs_by_run,
        available_run_tasks_by_run,
        Arc::new(HashMap::new()),
        persisted_outputs_by_workflow,
        &manager,
        tx,
    )
    .await;
    assert!(result.is_ok());

    let (_id, _job_name, params) = executor
        .captured
        .lock()
        .await
        .clone()
        .expect("captured execute params");
    let params = params.expect("params should be present");
    assert_eq!(params["task_input"]["from_ref"], 9);
}

#[tokio::test]
async fn test_execute_task_does_not_use_persisted_output_when_dependency_exists_in_run() {
    let mut task = sample_task_with_workflow(Some(serde_json::json!({
        "task_bindings": {
            "from_ref": {"ref": "tasks.dep.output.x"}
        }
    })));
    task.depends_on = vec!["dep".to_string()];

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::from([(task.run_id, HashMap::new())]));
    let available_run_tasks_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashSet::from([task.task_name.clone(), "dep".to_string()]),
    )]));
    let persisted_outputs_by_workflow = Arc::new(HashMap::from([(
        task.workflow_id,
        HashMap::from([("dep".to_string(), serde_json::json!({"x": 9}))]),
    )]));
    let executor = Arc::new(RecordingExecutor::new(false));
    let manager = RecordingManager {
        executor: executor.clone(),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(
        task,
        workflow_map,
        outputs_by_run,
        available_run_tasks_by_run,
        Arc::new(HashMap::new()),
        persisted_outputs_by_workflow,
        &manager,
        tx,
    )
    .await;
    let err = result.expect_err("binding should fail while in-run dependency output is missing");
    assert!(
        err.to_string()
            .contains("in this run but has no output yet"),
        "unexpected error: {err}"
    );
    assert!(
        executor.captured.lock().await.is_none(),
        "executor should not be called when binding resolution fails"
    );
}
