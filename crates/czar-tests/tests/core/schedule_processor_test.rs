use anyhow::Result;
use czar_core::database::{RunRepository, ScheduleRepository, WorkflowRepository};
use czar_core::schedule_processor::{
    next_scheduled_at, process_scheduled_triggers, process_scheduled_triggers_with_scope,
};
use czar_core::workflow_registry::{WorkflowRegistrationMetadata, with_registration_metadata};
use czar_store::SqliteDatabase;
use uuid::Uuid;

async fn setup_db() -> Result<SqliteDatabase> {
    let db = SqliteDatabase::new(":memory:").await?;
    db.run_migrations().await?;
    Ok(db)
}

fn init_tracing() -> tracing::dispatcher::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::set_default(subscriber)
}

fn make_temp_project_dir(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("czar-{label}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).expect("temp project dir");
    path
}

#[tokio::test]
async fn test_process_scheduled_triggers_returns_zero_when_none_due() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await?;

    let triggered = process_scheduled_triggers(&db).await?;
    assert_eq!(triggered, 0);

    Ok(())
}

#[tokio::test]
async fn test_process_scheduled_triggers_handles_invalid_cron() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await?;
    let workflow = db
        .create_workflow(
            "invalid_schedule",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            Some("not-a-cron"),
        )
        .await?;
    db.update_workflow_schedule(workflow.id, workflow.schedule.as_deref(), true)
        .await?;

    let triggered = process_scheduled_triggers(&db).await?;
    assert_eq!(triggered, 0);

    let runs = db.list_runs(Some(workflow.id)).await?;
    assert!(runs.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_process_scheduled_triggers_skips_enabled_workflow_without_schedule() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await?;
    let workflow = db
        .create_workflow(
            "missing_schedule",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            None,
        )
        .await?;
    db.update_workflow_schedule(workflow.id, None, true).await?;

    let triggered = process_scheduled_triggers(&db).await?;
    assert_eq!(triggered, 0);
    assert!(db.list_runs(Some(workflow.id)).await?.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_process_scheduled_triggers_creates_run_and_updates_schedule() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await?;
    let workflow = db
        .create_workflow(
            "valid_schedule",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            Some("* * * * * *"),
        )
        .await?;
    db.update_workflow_schedule(workflow.id, workflow.schedule.as_deref(), true)
        .await?;

    let triggered = process_scheduled_triggers(&db).await?;
    assert_eq!(triggered, 1);

    let runs = db.list_runs(Some(workflow.id)).await?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].triggered_by, "schedule");

    let refreshed = db.get_workflow_by_id(workflow.id).await?;
    assert!(refreshed.last_scheduled_at.is_some());
    assert!(refreshed.next_scheduled_at.is_some());

    Ok(())
}

#[tokio::test]
async fn test_process_scheduled_triggers_handles_create_run_error() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await?;
    let workflow = db
        .create_workflow(
            "create_run_error",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            Some("* * * * * *"),
        )
        .await?;
    db.update_workflow_schedule(workflow.id, workflow.schedule.as_deref(), true)
        .await?;

    sqlx::query("DROP TABLE runs").execute(db.pool()).await?;

    let triggered = process_scheduled_triggers(&db).await?;
    assert_eq!(triggered, 0);

    Ok(())
}

#[tokio::test]
async fn test_process_scheduled_triggers_logs_schedule_time_update_error() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await?;
    let workflow = db
        .create_workflow(
            "schedule_update_error",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            Some("* * * * * *"),
        )
        .await?;
    db.update_workflow_schedule(workflow.id, workflow.schedule.as_deref(), true)
        .await?;

    sqlx::query(
        r#"
        CREATE TRIGGER fail_schedule_time_update
        BEFORE UPDATE ON workflows
        BEGIN
            SELECT RAISE(ABORT, 'block schedule time update');
        END;
        "#,
    )
    .execute(db.pool())
    .await?;

    let triggered = process_scheduled_triggers(&db).await?;
    assert_eq!(triggered, 1);
    assert_eq!(db.list_runs(Some(workflow.id)).await?.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_process_scheduled_triggers_scopes_to_active_definitions_root() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await?;
    let current_project = make_temp_project_dir("schedule-current");
    let current_file = current_project.join("current.yaml");
    std::fs::write(&current_file, "name: current\n").expect("current file");

    let other_project = make_temp_project_dir("schedule-other");
    let other_file = other_project.join("other.yaml");
    std::fs::write(&other_file, "name: other\n").expect("other file");

    let stale_file = current_project.join("stale.yaml");
    std::fs::write(&stale_file, "name: stale\n").expect("stale file");

    let current = db
        .create_workflow(
            "current_scope",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            Some("* * * * * *"),
        )
        .await?;
    db.update_workflow_task_params(
        current.id,
        Some(with_registration_metadata(
            Some(serde_json::json!({"workflow_format": "v2"})),
            &WorkflowRegistrationMetadata::new(&current_project, &current_file),
        )),
    )
    .await?;
    db.update_workflow_schedule(current.id, current.schedule.as_deref(), true)
        .await?;

    let other = db
        .create_workflow(
            "other_scope",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            Some("* * * * * *"),
        )
        .await?;
    db.update_workflow_task_params(
        other.id,
        Some(with_registration_metadata(
            Some(serde_json::json!({"workflow_format": "v2"})),
            &WorkflowRegistrationMetadata::new(&other_project, &other_file),
        )),
    )
    .await?;
    db.update_workflow_schedule(other.id, other.schedule.as_deref(), true)
        .await?;

    let stale = db
        .create_workflow(
            "stale_scope",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            Some("* * * * * *"),
        )
        .await?;
    db.update_workflow_task_params(
        stale.id,
        Some(with_registration_metadata(
            Some(serde_json::json!({"workflow_format": "v2"})),
            &WorkflowRegistrationMetadata::new(&current_project, &stale_file),
        )),
    )
    .await?;
    db.update_workflow_schedule(stale.id, stale.schedule.as_deref(), true)
        .await?;
    std::fs::remove_file(&stale_file).expect("remove stale file");

    let triggered = process_scheduled_triggers_with_scope(&db, Some(&current_project)).await?;
    assert_eq!(triggered, 1);

    let current_runs = db.list_runs(Some(current.id)).await?;
    let other_runs = db.list_runs(Some(other.id)).await?;
    let stale_runs = db.list_runs(Some(stale.id)).await?;
    assert_eq!(current_runs.len(), 1);
    assert!(other_runs.is_empty());
    assert!(stale_runs.is_empty());

    Ok(())
}

#[test]
fn test_next_scheduled_at_accepts_five_and_six_field_cron() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-01-01T10:30:15Z")
        .unwrap()
        .to_utc();
    let expected = chrono::DateTime::parse_from_rfc3339("2026-01-02T02:00:00Z")
        .unwrap()
        .to_utc();

    assert_eq!(next_scheduled_at("0 2 * * *", now).unwrap(), expected);
    assert_eq!(next_scheduled_at("0 0 2 * * *", now).unwrap(), expected);
    assert!(next_scheduled_at("not a cron", now).is_err());
    // Valid syntax, but February 30th never comes.
    assert!(next_scheduled_at("0 0 30 2 *", now).is_err());
    assert!(next_scheduled_at("", now).is_err());
}

#[test]
fn test_next_scheduled_at_uses_posix_weekdays() {
    // 2026-01-01 is a Thursday.
    let now = chrono::DateTime::parse_from_rfc3339("2026-01-01T10:30:15Z")
        .unwrap()
        .to_utc();
    let next_monday = chrono::DateTime::parse_from_rfc3339("2026-01-05T02:00:00Z")
        .unwrap()
        .to_utc();
    let next_sunday = chrono::DateTime::parse_from_rfc3339("2026-01-04T02:00:00Z")
        .unwrap()
        .to_utc();

    assert_eq!(next_scheduled_at("0 2 * * 1", now).unwrap(), next_monday);
    assert_eq!(next_scheduled_at("0 0 2 * * 1", now).unwrap(), next_monday);
    assert_eq!(next_scheduled_at("0 2 * * 0", now).unwrap(), next_sunday);
    assert_eq!(next_scheduled_at("0 2 * * 7", now).unwrap(), next_sunday);
    assert_eq!(next_scheduled_at("0 2 * * MON", now).unwrap(), next_monday);
}
