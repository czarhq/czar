use super::*;

#[test]
fn test_resolve_job_name_prefers_params_over_task_default() {
    let with_command = sample_task_with_workflow(Some(serde_json::json!({
        "command": "echo hello"
    })));
    assert_eq!(resolve_job_name(&with_command), "echo hello");

    let with_job_name = sample_task_with_workflow(Some(serde_json::json!({
        "job_name": "overridden"
    })));
    assert_eq!(resolve_job_name(&with_job_name), "overridden");
}

#[test]
fn test_resolve_job_name_falls_back_to_task_joined_data() {
    let task = sample_task_with_workflow(None);
    assert_eq!(resolve_job_name(&task), "workflow-job");
}

#[test]
fn test_retry_backoff_seconds_caps() {
    assert_eq!(retry_backoff_seconds(0), 1);
    assert_eq!(retry_backoff_seconds(1), 1);
    assert_eq!(retry_backoff_seconds(2), 2);
    assert_eq!(retry_backoff_seconds(6), 32);
    assert_eq!(retry_backoff_seconds(10), 60);
    assert_eq!(retry_backoff_seconds(15), 60);
}

#[tokio::test]
async fn test_execute_task_rejects_non_pending_status() {
    let mut task = sample_task_with_workflow(None);
    task.task_status = TaskStatus::Running;

    let workflow_map = Arc::new(HashMap::from([(
        task.workflow_id,
        sample_workflow(task.workflow_id),
    )]));
    let outputs_by_run = Arc::new(HashMap::new());
    let manager = RecordingManager {
        executor: Arc::new(RecordingExecutor::new(false)),
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
    let err = result.expect_err("non-pending task must fail");
    assert!(err.to_string().contains("non-pending status"));
}

#[tokio::test]
async fn test_execute_task_errors_when_workflow_missing() {
    let task = sample_task_with_workflow(None);
    let workflow_map = Arc::new(HashMap::new());
    let outputs_by_run = Arc::new(HashMap::new());
    let manager = RecordingManager {
        executor: Arc::new(RecordingExecutor::new(false)),
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
    let err = result.expect_err("missing workflow must fail");
    assert!(err.to_string().contains("not found for task"));
}

#[tokio::test]
async fn test_execute_task_blocks_when_in_run_upstream_dependency_failed() {
    let mut task = sample_task_with_workflow(Some(serde_json::json!({
        "task_input": {"ok": true}
    })));
    task.depends_on = vec!["dep".to_string()];

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::new());
    let available_run_tasks_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashSet::from([task.task_name.clone(), "dep".to_string()]),
    )]));
    let dependency_statuses_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashMap::from([("dep".to_string(), TaskStatus::Failed)]),
    )]));
    let persisted_outputs_by_workflow = Arc::new(HashMap::new());
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
        dependency_statuses_by_run,
        persisted_outputs_by_workflow,
        &manager,
        tx,
    )
    .await;
    let err = result.expect_err("task should be blocked by upstream failure");
    assert!(
        err.to_string().contains("upstream dependency 'dep' failed"),
        "unexpected error: {err}"
    );
    assert!(
        executor.captured.lock().await.is_none(),
        "executor should not be called when upstream failed"
    );
}

#[tokio::test]
async fn test_execute_task_allows_failed_upstream_when_on_failure_run() {
    let mut task = sample_task_with_workflow(Some(serde_json::json!({
        "on_failure": "run",
        "task_bindings": {
            "dep_result": {"ref": "tasks.dep.output.result"}
        }
    })));
    task.depends_on = vec!["dep".to_string()];

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::new());
    let available_run_tasks_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashSet::from([task.task_name.clone(), "dep".to_string()]),
    )]));
    let dependency_statuses_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashMap::from([("dep".to_string(), TaskStatus::Failed)]),
    )]));
    let persisted_outputs_by_workflow = Arc::new(HashMap::new());
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
        dependency_statuses_by_run,
        persisted_outputs_by_workflow,
        &manager,
        tx,
    )
    .await;
    assert_eq!(
        result.expect("task should dispatch"),
        TaskOutcome::Dispatched("exec-123".to_string())
    );

    let (_id, _job_name, params) = executor
        .captured
        .lock()
        .await
        .clone()
        .expect("captured execute params");
    let params = params.expect("params should be present");
    assert!(params.get("task_input").is_some());
    assert!(params["task_input"]["dep_result"].is_null());
}

#[tokio::test]
async fn test_execute_task_binding_ref_requires_dependency() {
    let task = sample_task_with_workflow(Some(serde_json::json!({
        "task_bindings": {
            "x": {"ref": "tasks.other.output.value"}
        }
    })));

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::new());
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
    let err = result.expect_err("binding ref without dependency should fail");
    assert!(
        err.to_string().contains("not listed in depends_on"),
        "unexpected error: {err}"
    );
    assert!(
        executor.captured.lock().await.is_none(),
        "executor should not be called on binding resolution error"
    );
}

#[tokio::test]
async fn test_execute_task_resolves_parallel_and_foreach_context_refs() {
    let task = sample_task_with_workflow(Some(serde_json::json!({
        "parallel_index": 1,
        "parallel_count": 3,
        "foreach_index": 1,
        "foreach_count": 3,
        "foreach_item": {"name": "bravo"},
        "task_bindings": {
            "index": {"ref": "parallel.index"},
            "count": {"ref": "parallel.count"},
            "item_name": {"ref": "foreach.item.name"},
            "item_index": {"ref": "foreach.index"},
            "item_count": {"ref": "foreach.count"}
        }
    })));

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::new());
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

    let (_, _, params) = executor
        .captured
        .lock()
        .await
        .clone()
        .expect("captured execute params");
    let params = params.expect("params");
    assert_eq!(
        params["task_input"],
        serde_json::json!({
            "index": 1,
            "count": 3,
            "item_name": "bravo",
            "item_index": 1,
            "item_count": 3
        })
    );
}

#[tokio::test]
async fn test_execute_task_resolves_workflow_var_refs() {
    let task = sample_task_with_workflow(Some(serde_json::json!({
        "workflow_vars": {
            "GREETING": "hello",
            "platform.lib_ext": "so"
        },
        "task_bindings": {
            "greeting": {"ref": "vars.GREETING"},
            "ext": {"ref": "vars.platform.lib_ext"}
        }
    })));

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::new());
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

    let (_, _, params) = executor
        .captured
        .lock()
        .await
        .clone()
        .expect("captured execute params");
    let params = params.expect("params");
    assert_eq!(
        params["task_input"],
        serde_json::json!({
            "greeting": "hello",
            "ext": "so"
        })
    );
}
