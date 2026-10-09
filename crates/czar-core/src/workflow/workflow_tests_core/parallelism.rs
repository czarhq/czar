use super::*;

#[test]
fn test_validate_accepts_dynamic_parallelism_with_valid_dependencies() {
    let mut tasks = IndexMap::new();
    let mut discover = process_task();
    discover.output_type = Some(serde_json::json!({"count": "int", "items": ["str"]}));
    tasks.insert("discover".to_string(), discover);

    let mut process = process_task();
    process.depends_on = vec!["discover".to_string()];
    process.parallel = Some(ParallelSpec::Ref("discover.count".to_string()));
    tasks.insert("process".to_string(), process);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(wf.validate().is_ok(), "dynamic parallel should validate");

    let mut tasks = IndexMap::new();
    let mut discover = process_task();
    discover.output_type = Some(serde_json::json!({"items": ["str"]}));
    tasks.insert("discover".to_string(), discover);
    let mut foreach_task = process_task();
    foreach_task.depends_on = vec!["discover".to_string()];
    foreach_task.foreach = Some("discover.items".to_string());
    tasks.insert("foreach_task".to_string(), foreach_task);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(wf.validate().is_ok(), "foreach should validate");

    let mut tasks = IndexMap::new();
    let mut discover = process_task();
    discover.output_type = Some(serde_json::json!({"items": "list[str]"}));
    tasks.insert("discover".to_string(), discover);
    let mut foreach_task = process_task();
    foreach_task.depends_on = vec!["discover".to_string()];
    foreach_task.foreach = Some("discover.items".to_string());
    tasks.insert("foreach_task".to_string(), foreach_task);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(
        wf.validate().is_ok(),
        "foreach should validate with list shorthand"
    );
}

#[test]
fn test_validate_rejects_invalid_dynamic_parallelism_shapes() {
    let mut tasks = IndexMap::new();
    let mut discover = process_task();
    discover.output_type = Some(serde_json::json!({"count": "str", "items": {"name": "str"}}));
    tasks.insert("discover".to_string(), discover);

    let mut process = process_task();
    process.depends_on = vec!["discover".to_string()];
    process.parallel = Some(ParallelSpec::Ref("discover.count".to_string()));
    tasks.insert("process".to_string(), process);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(matches!(
        wf.validate(),
        Err(WorkflowValidationError::Custom(msg))
            if msg.contains("must resolve to an int")
    ));

    let mut tasks = IndexMap::new();
    let mut discover = process_task();
    discover.output_type = Some(serde_json::json!({"items": {"name": "str"}}));
    tasks.insert("discover".to_string(), discover);
    let mut foreach_task = process_task();
    foreach_task.depends_on = vec!["discover".to_string()];
    foreach_task.foreach = Some("discover.items".to_string());
    tasks.insert("foreach_task".to_string(), foreach_task);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(matches!(
        wf.validate(),
        Err(WorkflowValidationError::Custom(msg))
            if msg.contains("must resolve to an array")
    ));
}

#[test]
fn test_validate_rejects_invalid_parallel_mode_combinations() {
    let mut tasks = IndexMap::new();
    let mut process = process_task();
    process.parallel_mode = ParallelMode::CloudRunTasks;
    process.parallel = Some(ParallelSpec::Count(4));
    tasks.insert("process".to_string(), process);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(matches!(
        wf.validate(),
        Err(WorkflowValidationError::Custom(msg))
            if msg.contains("parallel_mode: cloud_run_tasks")
    ));
}
