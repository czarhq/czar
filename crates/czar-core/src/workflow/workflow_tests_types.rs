use super::*;
use crate::error::WorkflowValidationError;

fn process_task() -> TaskDefinition {
    TaskDefinition {
        executor: ExecutorKind::Process,
        command: Some("echo ok".to_string()),
        ..Default::default()
    }
}

#[test]
fn test_validate_type_schema_rejects_unknown_primitive() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({"data": "wat"}));
    tasks.insert("a".to_string(), a);

    let mut b = process_task();
    b.depends_on = vec!["a".to_string()];
    b.input_type = Some(serde_json::json!({"upstream": {"a": {"data": "str"}}}));
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
    let err = wf.validate().unwrap_err();
    assert!(matches!(
        err,
        WorkflowValidationError::InvalidTypeSchema { .. }
    ));
    assert!(err.to_string().contains("wat"));
}

#[test]
fn test_validate_type_schema_accepts_all_primitives() {
    for prim in &[
        "str",
        "int",
        "float",
        "bool",
        "date",
        "datetime_notz",
        "datetime_tz",
        "datetime_utc",
    ] {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.output_type = Some(serde_json::json!({"field": prim}));
        tasks.insert("a".to_string(), a);

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
            "primitive '{}' should be valid",
            prim
        );
    }
}

#[test]
fn test_validate_type_schema_accepts_array_types() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({"users": ["str"], "scores": ["float"]}));
    tasks.insert("a".to_string(), a);

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
fn test_validate_type_schema_accepts_array_string_shorthand() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({
        "users": "list[str]",
        "scores": "Array[float]",
        "ids": "str[]",
        "records": "list[types.Record]"
    }));
    tasks.insert("a".to_string(), a);

    let mut types = BTreeMap::new();
    types.insert(
        "Record".to_string(),
        serde_json::json!({
            "id": "int",
            "name": "str"
        }),
    );

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types,
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    assert!(wf.validate().is_ok());
}

#[test]
fn test_validate_type_schema_rejects_empty_array() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({"items": []}));
    tasks.insert("a".to_string(), a);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    let err = wf.validate().unwrap_err();
    assert!(matches!(
        err,
        WorkflowValidationError::InvalidTypeSchema { .. }
    ));
    assert!(err.to_string().contains("exactly one element"));
}

#[test]
fn test_validate_type_schema_rejects_multi_element_array() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({"items": ["str", "int"]}));
    tasks.insert("a".to_string(), a);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    let err = wf.validate().unwrap_err();
    assert!(matches!(
        err,
        WorkflowValidationError::InvalidTypeSchema { .. }
    ));
}

#[test]
fn test_validate_type_schema_accepts_nested_objects() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({
        "user": {
            "name": "str",
            "age": "int",
            "tags": ["str"]
        }
    }));
    tasks.insert("a".to_string(), a);

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
fn test_validate_type_schema_rejects_numeric_values() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({"count": 42}));
    tasks.insert("a".to_string(), a);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    let err = wf.validate().unwrap_err();
    assert!(matches!(
        err,
        WorkflowValidationError::InvalidTypeSchema { .. }
    ));
}

#[test]
fn test_validate_type_schema_rejects_array_string_without_element_type() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({"items": "list"}));
    tasks.insert("a".to_string(), a);

    let wf = Workflow {
        name: "wf".to_string(),
        description: None,
        schedule: None,
        types: BTreeMap::new(),
        tasks,
        hooks: IndexMap::new(),
        vars: std::collections::BTreeMap::new(),
    };
    let err = wf.validate().unwrap_err();
    assert!(matches!(
        err,
        WorkflowValidationError::InvalidTypeSchema { .. }
    ));
    assert!(
        err.to_string()
            .contains("array type requires an element type")
    );
}

#[test]
fn test_validate_type_schema_accepts_date_and_datetimes() {
    let mut tasks = IndexMap::new();
    let mut a = process_task();
    a.output_type = Some(serde_json::json!({
        "created": "date",
        "updated_local": "datetime_notz",
        "updated_tz": "datetime_tz",
        "updated_utc": "datetime_utc",
        "timestamps": ["datetime_utc"]
    }));
    tasks.insert("a".to_string(), a);

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
