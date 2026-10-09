use super::*;
use tempfile::TempDir;

#[test]
fn test_workflow_definition_resolve_flow_merges_dependencies_and_overrides() {
    let yaml = r#"
name: wf_v2
tasks:
  extract:
    executor: process
    command: "echo extract"
    env:
      LOG_LEVEL: info
    output_type:
      result: str
    timeout: 60
  transform:
    executor: process
    command: "echo transform"
    depends_on: [extract]
    input_type:
      result: str
    inputs:
      result:
        ref: tasks.extract.output.result
    timeout: 90
    retry_delay: 5
    on_failure: skip
flows:
  default:
    tasks:
      extract: {}
      transform:
        timeout: 300
        retry_delay: 30
        on_failure: run
        env:
          LOG_LEVEL: debug
"#;

    let definition = WorkflowDefinition::parse_yaml(yaml).expect("parse");
    let resolved = definition
        .resolve_flow(Some("default"))
        .expect("resolve default flow");

    assert_eq!(resolved.tasks.len(), 2);
    assert_eq!(resolved.tasks["extract"].depends_on, Vec::<String>::new());
    assert_eq!(resolved.tasks["extract"].timeout, 60);
    assert_eq!(
        resolved.tasks["extract"].env,
        serde_json::json!({"LOG_LEVEL": "info"})
    );
    assert_eq!(
        resolved.tasks["transform"].depends_on,
        vec!["extract".to_string()]
    );
    assert_eq!(resolved.tasks["transform"].timeout, 300);
    assert_eq!(resolved.tasks["transform"].retry_delay, Some(30));
    assert_eq!(resolved.tasks["transform"].on_failure, FailurePolicy::Run);
    assert_eq!(
        resolved.tasks["transform"].env,
        serde_json::json!({"LOG_LEVEL": "debug"})
    );
}

#[test]
fn test_workflow_definition_without_explicit_flow_runs_all_tasks() {
    let yaml = r#"
name: wf_v2
tasks:
  a:
    executor: process
    command: "echo a"
    output_type:
      result: str
  b:
    executor: process
    command: "echo b"
    depends_on: [a]
    input_type:
      result: str
    inputs:
      result:
        ref: tasks.a.output.result
flows:
  quick:
    tasks:
      - b
"#;

    let definition = WorkflowDefinition::parse_yaml(yaml).expect("parse");
    definition.validate_catalog().expect("must validate");
    let resolved = definition.resolve_flow(None).expect("resolve flow");
    assert_eq!(resolved.tasks.len(), 2);
    assert_eq!(resolved.tasks["b"].depends_on, vec!["a".to_string()]);
}

#[test]
fn test_workflow_definition_flow_tasks_star_shorthand() {
    let yaml = r#"
name: wf_star
tasks:
  a:
    executor: process
    command: "echo a"
    output_type:
      result: str
  b:
    executor: process
    command: "echo b"
flows:
  default:
    tasks: "*"
"#;

    let definition = WorkflowDefinition::parse_yaml(yaml).expect("parse");
    let resolved = definition.resolve_flow(Some("default")).expect("resolve");
    assert_eq!(resolved.tasks.len(), 2);
    assert!(resolved.tasks.contains_key("a"));
    assert!(resolved.tasks.contains_key("b"));
}

#[test]
fn test_workflow_definition_flow_tasks_map_allows_empty_entries() {
    let yaml = r#"
name: wf_empty_entries
tasks:
  a:
    executor: process
    command: "echo a"
    output_type:
      result: str
  b:
    executor: process
    command: "echo b"
    depends_on: [a]
    input_type:
      result: str
    inputs:
      result:
        ref: tasks.a.output.result
flows:
  branch:
    tasks:
      a:
      b:
"#;

    let definition = WorkflowDefinition::parse_yaml(yaml).expect("parse");
    let resolved = definition.resolve_flow(Some("branch")).expect("resolve");
    assert_eq!(resolved.tasks.len(), 2);
    assert_eq!(resolved.tasks["b"].depends_on, vec!["a".to_string()]);
}

#[test]
fn test_workflow_definition_expands_dbt_imports_into_tasks_and_flows() {
    let dir = TempDir::new().expect("temp dir");
    let project_dir = dir.path().join("transform").join("target");
    std::fs::create_dir_all(&project_dir).expect("create manifest dir");
    std::fs::write(
        project_dir.join("manifest.json"),
        r#"
{
  "metadata": { "project_name": "analytics" },
  "nodes": {
    "seed.analytics.raw_orders": {
      "unique_id": "seed.analytics.raw_orders",
      "name": "raw_orders",
      "resource_type": "seed",
      "path": "seeds/raw_orders.csv",
      "fqn": ["analytics", "raw_orders"],
      "depends_on": { "nodes": [] },
      "config": {}
    },
    "model.analytics.staging.stg_orders": {
      "unique_id": "model.analytics.staging.stg_orders",
      "name": "stg_orders",
      "resource_type": "model",
      "path": "models/staging/stg_orders.sql",
      "fqn": ["analytics", "staging", "stg_orders"],
      "depends_on": { "nodes": ["seed.analytics.raw_orders"] },
      "config": { "materialized": "view" }
    },
    "model.analytics.marts.daily_revenue": {
      "unique_id": "model.analytics.marts.daily_revenue",
      "name": "daily_revenue",
      "resource_type": "model",
      "path": "models/marts/daily_revenue.sql",
      "fqn": ["analytics", "marts", "daily_revenue"],
      "depends_on": { "nodes": ["model.analytics.staging.stg_orders"] },
      "config": { "materialized": "table" }
    },
    "test.analytics.assert_daily_revenue": {
      "unique_id": "test.analytics.assert_daily_revenue",
      "name": "assert_daily_revenue",
      "resource_type": "test",
      "path": "tests/assert_daily_revenue.sql",
      "fqn": ["analytics", "assert_daily_revenue"],
      "depends_on": { "nodes": ["model.analytics.marts.daily_revenue"] },
      "config": {}
    }
  }
}
"#,
    )
    .expect("write manifest");

    let yaml = r#"
name: dbt_import
imports:
  - dbt: transform
    label: analytics
    depends_on: [extract]
tasks:
  extract:
    executor: process
    command: "echo extract"
  audit:
    executor: process
    command: "echo audit"
  dbt.analytics.marts.daily_revenue:
    depends_on: [audit]
"#;

    let definition = WorkflowDefinition::parse_yaml_with_root(yaml, dir.path()).expect("parse");
    definition.validate_catalog().expect("validate");
    let resolved = definition.resolve_flow(None).expect("resolve");

    assert!(resolved.tasks.contains_key("dbt.analytics.seed.raw_orders"));
    assert!(
        resolved
            .tasks
            .contains_key("dbt.analytics.staging.stg_orders")
    );
    assert!(
        resolved
            .tasks
            .contains_key("dbt.analytics.marts.daily_revenue")
    );
    assert!(
        resolved
            .tasks
            .contains_key("dbt.analytics.test.assert_daily_revenue")
    );

    assert_eq!(
        resolved.tasks["dbt.analytics.seed.raw_orders"].depends_on,
        vec!["extract".to_string()]
    );
    assert_eq!(
        resolved.tasks["dbt.analytics.staging.stg_orders"].depends_on,
        vec!["dbt.analytics.seed.raw_orders".to_string()]
    );
    assert_eq!(
        resolved.tasks["dbt.analytics.marts.daily_revenue"].depends_on,
        vec![
            "dbt.analytics.staging.stg_orders".to_string(),
            "audit".to_string()
        ]
    );
    assert_eq!(
        resolved.tasks["dbt.analytics.test.assert_daily_revenue"].depends_on,
        vec!["dbt.analytics.marts.daily_revenue".to_string()]
    );
    assert!(definition.flows.contains_key("dbt.analytics.all"));
    assert!(definition.flows.contains_key("dbt.analytics.path.marts"));
    assert!(
        definition
            .flows
            .contains_key("dbt.analytics.resource.tests")
    );
}

#[test]
fn test_workflow_definition_loads_custom_executor_registry_from_project_config() {
    let dir = TempDir::new().expect("temp dir");
    std::fs::write(
        dir.path().join("czar.yaml"),
        r#"
executors:
  sqlite_query:
    type: custom_exec.sqlite:SqliteQueryExecutor
    config:
      default_limit: 50
"#,
    )
    .expect("write czar.yaml");
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"custom-exec\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
    std::fs::write(dir.path().join("query.yaml"), "sql: select 1 as n\n")
        .expect("write config file");

    let yaml = r#"
name: custom_exec
tasks:
  query:
    executor: sqlite_query
    config_file: query.yaml
"#;

    let definition = WorkflowDefinition::parse_yaml_with_root(yaml, dir.path()).expect("parse");
    definition.validate_catalog().expect("validate");
    let resolved = definition.resolve_flow(None).expect("resolve");
    let task = &resolved.tasks["query"];
    assert_eq!(
        task.executor,
        ExecutorKind::Custom("sqlite_query".to_string())
    );
    assert_eq!(
        task.config_file.as_deref(),
        Some(std::path::Path::new("query.yaml"))
    );
    let custom = task
        .custom_executor
        .as_ref()
        .expect("resolved custom executor metadata");
    assert_eq!(custom.target, "custom_exec.sqlite:SqliteQueryExecutor");
    assert_eq!(custom.config, serde_json::json!({"default_limit": 50}));

    let compiled = resolved.compile(dir.path()).expect("compile");
    let rows = crate::compiled::build_workflow_tasks(&compiled);
    assert_eq!(rows[0].executor_type, "custom:sqlite_query");
    assert_eq!(
        rows[0].params.get("custom_executor_alias"),
        Some(&serde_json::Value::String("sqlite_query".to_string()))
    );
}
