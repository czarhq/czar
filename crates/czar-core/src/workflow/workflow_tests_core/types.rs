use super::*;
use tempfile::NamedTempFile;

#[test]
fn test_validate_types_allows_ordering_only_dependencies_without_output_type() {
    let mut tasks = IndexMap::new();
    let a = process_task();
    let mut b = process_task();
    b.depends_on = vec!["a".to_string()];
    tasks.insert("a".to_string(), a);
    tasks.insert("b".to_string(), b);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(wf.validate().is_ok());
}

#[test]
fn test_validate_types_allows_ordering_only_dependencies_without_input_type() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({"x": "int"}));
    let mut b = process_task();
    b.depends_on = vec!["a".to_string()];
    tasks.insert("a".to_string(), a);
    tasks.insert("b".to_string(), b);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(wf.validate().is_ok());
}

#[test]
fn test_validate_types_requires_input_type_when_inputs_are_present() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({"x": "int"}));
    let mut b = process_task();
    b.depends_on = vec!["a".to_string()];
    b.inputs = serde_json::json!({
        "x": {
            "ref": "tasks.a.output.x"
        }
    });
    tasks.insert("a".to_string(), a);
    tasks.insert("b".to_string(), b);

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
        matches!(wf.validate(), Err(WorkflowValidationError::Custom(msg)) if msg.contains("defines non-empty 'inputs' but is missing 'input_type'"))
    );
}

#[test]
fn test_validate_types_accepts_valid_typed_workflow() {
    let mut tasks = IndexMap::new();
    let mut extract = process_task();
    extract.output_type = Some(serde_json::json!({"users": ["str"], "count": "int"}));
    let mut transform = process_task();
    transform.depends_on = vec!["extract".to_string()];
    transform.input_type =
        Some(serde_json::json!({"upstream": {"extract": {"users": ["str"], "count": "int"}}}));
    transform.inputs = serde_json::json!({
        "upstream": {
            "ref": "tasks.extract.output"
        }
    });
    tasks.insert("extract".to_string(), extract);
    tasks.insert("transform".to_string(), transform);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(wf.validate().is_ok());
}

#[test]
fn test_validate_types_not_required_for_independent_tasks() {
    let mut tasks = IndexMap::new();
    tasks.insert("a".to_string(), process_task());
    tasks.insert("b".to_string(), process_task());

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(wf.validate().is_ok());
}

#[test]
fn test_validate_types_leaf_task_output_type_optional() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({"x": "int"}));
    let mut b = process_task();
    b.depends_on = vec!["a".to_string()];
    b.input_type = Some(serde_json::json!({"upstream": {"a": {"x": "int"}}}));
    b.inputs = serde_json::json!({
        "upstream": {
            "ref": "tasks.a.output"
        }
    });
    tasks.insert("a".to_string(), a);
    tasks.insert("b".to_string(), b);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(wf.validate().is_ok());
}

#[test]
fn test_load_parses_typed_workflow_yaml() {
    let mut file = NamedTempFile::new().expect("tempfile should be created");
    let yaml = r#"
name: typed_workflow
tasks:
  extract:
    executor: process
    command: "echo extract"
    output_type:
      users: [str]
      count: int
  transform:
    executor: process
    command: "echo transform"
    depends_on: [extract]
    input_type:
      upstream:
        extract:
          users: [str]
          count: int
    inputs:
      upstream:
        ref: tasks.extract.output
"#;
    std::io::Write::write_all(&mut file, yaml.as_bytes()).expect("yaml content should be written");

    let loaded = Workflow::load(file.path()).expect("typed workflow yaml should load");
    assert_eq!(loaded.name, "typed_workflow");
    assert!(loaded.tasks["extract"].output_type.is_some());
    assert!(loaded.tasks["transform"].input_type.is_some());
    let output_type = loaded.tasks["extract"].output_type.as_ref().unwrap();
    assert_eq!(output_type["count"], "int");
}
