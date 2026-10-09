use super::*;

#[test]
fn test_uses_api_for_pre_db_commands() {
    let validate = Commands::ValidateWorkflow(ValidateWorkflow {
        file: "wf.yaml".to_string(),
    });
    assert!(validate.uses_api());

    let schema = Commands::Schema(Schema {
        action: schema::SchemaAction::Export(schema::SchemaExport {
            file: "task.py".to_string(),
            function: "main".to_string(),
            project_root: None,
        }),
    });
    assert!(schema.uses_api());

    let non_api = Commands::Init(Init);
    assert!(!non_api.uses_api());

    let logs = Commands::Logs(Logs {
        run_id: "run-id".to_string(),
        task: None,
        follow: false,
        poll_interval_ms: 1000,
    });
    assert!(!logs.uses_api());
}
