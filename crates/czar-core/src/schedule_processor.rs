use anyhow::Result;
use chrono::{DateTime, Utc};
use croner::errors::CronError;
use croner::parser::{CronParser, Seconds};
use tracing::{error, info};

use crate::database::Database;
use crate::models::Workflow;
use crate::workflow_registry::{
    active_definitions_root_from_env, workflow_is_active_for_definitions_root,
};

/// Process workflows that have schedules enabled and are due for triggering
pub async fn process_scheduled_triggers<D: Database>(db: &D) -> Result<usize> {
    let active_definitions_root = active_definitions_root_from_env();
    process_scheduled_triggers_with_scope(db, active_definitions_root.as_deref()).await
}

pub async fn process_scheduled_triggers_with_scope<D: Database>(
    db: &D,
    definitions_root: Option<&std::path::Path>,
) -> Result<usize> {
    let mut workflows = db.get_due_scheduled_workflows().await?;
    if let Some(definitions_root) = definitions_root {
        workflows
            .retain(|workflow| workflow_is_active_for_definitions_root(workflow, definitions_root));
    }

    if workflows.is_empty() {
        return Ok(0);
    }

    let now = Utc::now();
    let mut triggered_count = 0;

    for workflow in workflows {
        if let Some(schedule_str) = workflow.schedule.as_deref() {
            triggered_count += process_workflow_schedule(db, &workflow, schedule_str, now).await;
        }
    }

    if triggered_count > 0 {
        info!("Triggered {} scheduled workflow runs", triggered_count);
    }

    Ok(triggered_count)
}

/// Next fire time of a cron `schedule` after `now`: standard POSIX cron
/// (weekday 0 or 7 = Sunday), with an optional leading seconds field. A
/// schedule that never fires (e.g. `0 0 30 2 *`) is an error: an empty next
/// time would count as due on every check.
pub fn next_scheduled_at(schedule: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>, CronError> {
    CronParser::builder()
        .seconds(Seconds::Optional)
        .build()
        .parse(schedule.trim())?
        .find_next_occurrence(&now, false)
}

async fn process_workflow_schedule<D: Database>(
    db: &D,
    workflow: &Workflow,
    schedule_str: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> usize {
    let next_scheduled_at = match next_scheduled_at(schedule_str, now) {
        Ok(next) => next,
        Err(e) => {
            let msg = format!(
                "Invalid cron expression '{}' for workflow {}: {}",
                schedule_str, workflow.name, e
            );
            error!("{}", msg);
            return 0;
        }
    };

    // Create run for this scheduled trigger
    match db.create_run(workflow.id, "schedule").await {
        Ok(run) => {
            let msg = format!(
                "Created scheduled run {} for workflow {}",
                run.id, workflow.name
            );
            info!("{}", msg);
        }
        Err(e) => {
            let msg = format!(
                "Failed to create scheduled run for workflow {}: {}",
                workflow.name, e
            );
            error!("{}", msg);
            return 0;
        }
    }

    // Update workflow schedule times
    if let Err(e) = db
        .update_workflow_schedule_times(workflow.id, now, Some(next_scheduled_at))
        .await
    {
        let msg = format!(
            "Failed to update schedule times for workflow {}: {}",
            workflow.name, e
        );
        error!("{}", msg);
    }

    1
}
