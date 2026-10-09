use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use czar_core::models::{
    DeferredJob, DeferredJobStatus, Run, RunStatus, Task, TaskStatus, TaskWithWorkflow, Workflow,
    WorkflowSnapshot, WorkflowTask, WorkflowTaskState,
};

use crate::client::RowMap;

pub(crate) fn workflow(row: RowMap) -> Result<Workflow> {
    Ok(Workflow {
        id: uuid(&row, "id")?,
        name: string(&row, "name")?,
        description: opt_string(&row, "description")?,
        job_name: string(&row, "job_name")?,
        region: string(&row, "region")?,
        project: string(&row, "project")?,
        executor_type: string(&row, "executor_type")?,
        task_params: opt_json(&row, "task_params")?.map(Into::into),
        schedule: opt_string(&row, "schedule")?,
        schedule_enabled: bool_value(&row, "schedule_enabled")?,
        last_scheduled_at: opt_datetime(&row, "last_scheduled_at")?,
        next_scheduled_at: opt_datetime(&row, "next_scheduled_at")?,
        current_snapshot_id: opt_uuid(&row, "current_snapshot_id")?,
        created_at: datetime(&row, "created_at")?,
        updated_at: datetime(&row, "updated_at")?,
    })
}

pub(crate) fn workflow_snapshot(row: RowMap) -> Result<WorkflowSnapshot> {
    Ok(WorkflowSnapshot {
        id: uuid(&row, "id")?,
        workflow_id: uuid(&row, "workflow_id")?,
        content_hash: string(&row, "content_hash")?,
        tasks_json: json(&row, "tasks_json")?.into(),
        source_yaml: opt_string(&row, "source_yaml")?,
        created_at: datetime(&row, "created_at")?,
    })
}

pub(crate) fn run(row: RowMap) -> Result<Run> {
    Ok(Run {
        id: uuid(&row, "id")?,
        workflow_id: uuid(&row, "workflow_id")?,
        snapshot_id: opt_uuid(&row, "snapshot_id")?,
        task_selection: opt_json(&row, "task_selection")?.map(Into::into),
        status: run_status(&row, "status")?,
        triggered_by: string(&row, "triggered_by")?,
        started_at: opt_datetime(&row, "started_at")?,
        finished_at: opt_datetime(&row, "finished_at")?,
        error: opt_string(&row, "error")?,
        created_at: datetime(&row, "created_at")?,
    })
}

pub(crate) fn task(row: RowMap) -> Result<Task> {
    Ok(Task {
        id: uuid(&row, "id")?,
        run_id: uuid(&row, "run_id")?,
        task_index: i32_value(&row, "task_index")?,
        task_name: string(&row, "task_name")?,
        executor_type: string(&row, "executor_type")?,
        depends_on: depends_on(&row, "depends_on")?,
        status: task_status(&row, "status")?,
        attempts: i32_value(&row, "attempts")?,
        max_retries: i32_value(&row, "max_retries")?,
        timeout_seconds: opt_i32(&row, "timeout_seconds")?,
        retry_at: opt_datetime(&row, "retry_at")?,
        execution_name: opt_string(&row, "execution_name")?,
        params: opt_json(&row, "params")?.map(Into::into),
        output: opt_json(&row, "output")?.map(Into::into),
        logs: opt_string(&row, "logs")?,
        error: opt_string(&row, "error")?,
        progress: opt_json(&row, "progress")?.map(Into::into),
        dispatched_at: opt_datetime(&row, "dispatched_at")?,
        started_at: opt_datetime(&row, "started_at")?,
        finished_at: opt_datetime(&row, "finished_at")?,
        created_at: datetime(&row, "created_at")?,
    })
}

pub(crate) fn workflow_task(row: RowMap) -> Result<WorkflowTask> {
    Ok(WorkflowTask {
        id: uuid(&row, "id")?,
        workflow_id: uuid(&row, "workflow_id")?,
        task_index: i32_value(&row, "task_index")?,
        task_name: string(&row, "task_name")?,
        executor_type: string(&row, "executor_type")?,
        depends_on: depends_on(&row, "depends_on")?,
        params: opt_json(&row, "params")?.map(Into::into),
        signature: opt_json(&row, "signature")?.map(Into::into),
        created_at: datetime(&row, "created_at")?,
    })
}

pub(crate) fn task_with_workflow(row: RowMap) -> Result<TaskWithWorkflow> {
    Ok(TaskWithWorkflow {
        task_id: uuid(&row, "task_id")?,
        run_id: uuid(&row, "run_id")?,
        task_index: i32_value(&row, "task_index")?,
        task_name: string(&row, "task_name")?,
        executor_type: string(&row, "executor_type")?,
        depends_on: depends_on(&row, "depends_on")?,
        task_status: task_status(&row, "task_status")?,
        attempts: i32_value(&row, "attempts")?,
        max_retries: i32_value(&row, "max_retries")?,
        timeout_seconds: opt_i32(&row, "timeout_seconds")?,
        retry_at: opt_datetime(&row, "retry_at")?,
        execution_name: opt_string(&row, "execution_name")?,
        params: opt_json(&row, "params")?.map(Into::into),
        workflow_id: uuid(&row, "workflow_id")?,
        job_name: string(&row, "job_name")?,
        project: string(&row, "project")?,
        region: string(&row, "region")?,
    })
}

pub(crate) fn workflow_task_state(row: RowMap) -> Result<WorkflowTaskState> {
    Ok(WorkflowTaskState {
        id: uuid(&row, "id")?,
        workflow_id: uuid(&row, "workflow_id")?,
        task_name: string(&row, "task_name")?,
        last_status: task_status(&row, "last_status")?,
        last_run_id: uuid(&row, "last_run_id")?,
        last_task_id: uuid(&row, "last_task_id")?,
        last_output: opt_json(&row, "last_output")?.map(Into::into),
        last_error: opt_string(&row, "last_error")?,
        last_finished_at: opt_datetime(&row, "last_finished_at")?,
        last_success_at: opt_datetime(&row, "last_success_at")?,
        last_failure_at: opt_datetime(&row, "last_failure_at")?,
        updated_at: datetime(&row, "updated_at")?,
    })
}

pub(crate) fn deferred_job(row: RowMap) -> Result<DeferredJob> {
    Ok(DeferredJob {
        id: uuid(&row, "id")?,
        task_id: uuid(&row, "task_id")?,
        service_type: string(&row, "service_type")?,
        job_id: string(&row, "job_id")?,
        job_data: json(&row, "job_data")?.into(),
        status: deferred_job_status(&row, "status")?,
        error: opt_string(&row, "error")?,
        created_at: datetime(&row, "created_at")?,
        started_at: opt_datetime(&row, "started_at")?,
        last_polled_at: opt_datetime(&row, "last_polled_at")?,
        finished_at: opt_datetime(&row, "finished_at")?,
    })
}

pub(crate) fn string(row: &RowMap, key: &str) -> Result<String> {
    match required(row, key)? {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(if *value { "1" } else { "0" }.to_string()),
        other => bail!("expected string-compatible value for {key}, got {other}"),
    }
}

pub(crate) fn opt_string(row: &RowMap, key: &str) -> Result<Option<String>> {
    match row.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(Value::Number(value)) => Ok(Some(value.to_string())),
        Some(Value::Bool(value)) => Ok(Some(if *value { "1" } else { "0" }.to_string())),
        Some(other) => bail!("expected optional string-compatible value for {key}, got {other}"),
    }
}

pub(crate) fn json(row: &RowMap, key: &str) -> Result<Value> {
    let value = required(row, key)?;
    json_from_value(value, key)
}

pub(crate) fn opt_json(row: &RowMap, key: &str) -> Result<Option<Value>> {
    match row.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(json_from_value(value, key)?)),
    }
}

pub(crate) fn uuid(row: &RowMap, key: &str) -> Result<Uuid> {
    Uuid::parse_str(&string(row, key)?).with_context(|| format!("invalid UUID for {key}"))
}

pub(crate) fn opt_uuid(row: &RowMap, key: &str) -> Result<Option<Uuid>> {
    opt_string(row, key)?
        .map(|value| Uuid::parse_str(&value).with_context(|| format!("invalid UUID for {key}")))
        .transpose()
}

pub(crate) fn datetime(row: &RowMap, key: &str) -> Result<DateTime<Utc>> {
    parse_datetime(&string(row, key)?).with_context(|| format!("invalid datetime for {key}"))
}

pub(crate) fn opt_datetime(row: &RowMap, key: &str) -> Result<Option<DateTime<Utc>>> {
    opt_string(row, key)?
        .map(|value| parse_datetime(&value).with_context(|| format!("invalid datetime for {key}")))
        .transpose()
}

pub(crate) fn bool_value(row: &RowMap, key: &str) -> Result<bool> {
    match required(row, key)? {
        Value::Bool(value) => Ok(*value),
        Value::Number(value) => Ok(value.as_i64().unwrap_or_default() != 0),
        Value::String(value) => match value.as_str() {
            "1" | "true" | "TRUE" => Ok(true),
            "0" | "false" | "FALSE" => Ok(false),
            _ => bail!("invalid bool string for {key}: {value}"),
        },
        other => bail!("expected bool-compatible value for {key}, got {other}"),
    }
}

pub(crate) fn i32_value(row: &RowMap, key: &str) -> Result<i32> {
    let value = match required(row, key)? {
        Value::Number(value) => value
            .as_i64()
            .ok_or_else(|| anyhow!("invalid integer for {key}"))?,
        Value::String(value) => value.parse::<i64>()?,
        other => bail!("expected integer-compatible value for {key}, got {other}"),
    };

    i32::try_from(value).with_context(|| format!("integer out of range for {key}"))
}

pub(crate) fn opt_i32(row: &RowMap, key: &str) -> Result<Option<i32>> {
    match row.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => Ok(Some(i32_value(row, key)?)),
    }
}

pub(crate) fn depends_on(row: &RowMap, key: &str) -> Result<Vec<String>> {
    match row.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| anyhow!("depends_on entry for {key} must be a string"))
            })
            .collect(),
        Some(Value::String(value)) => {
            serde_json::from_str(value).with_context(|| format!("invalid JSON array for {key}"))
        }
        Some(other) => bail!("expected JSON array for {key}, got {other}"),
    }
}

pub(crate) fn task_status(row: &RowMap, key: &str) -> Result<TaskStatus> {
    TaskStatus::parse(&string(row, key)?).ok_or_else(|| anyhow!("invalid task status for {key}"))
}

pub(crate) fn run_status(row: &RowMap, key: &str) -> Result<RunStatus> {
    RunStatus::parse(&string(row, key)?).ok_or_else(|| anyhow!("invalid run status for {key}"))
}

pub(crate) fn deferred_job_status(row: &RowMap, key: &str) -> Result<DeferredJobStatus> {
    DeferredJobStatus::parse(&string(row, key)?)
        .ok_or_else(|| anyhow!("invalid deferred job status for {key}"))
}

fn required<'a>(row: &'a RowMap, key: &str) -> Result<&'a Value> {
    row.get(key)
        .with_context(|| format!("missing column {key} in D1 response row"))
}

fn json_from_value(value: &Value, key: &str) -> Result<Value> {
    match value {
        Value::Null => Ok(Value::Null),
        Value::String(value) => serde_json::from_str(value)
            .with_context(|| format!("invalid JSON string value for {key}")),
        Value::Object(_) | Value::Array(_) => Ok(value.clone()),
        other => Ok(other.clone()),
    }
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Ok(parsed.with_timezone(&Utc));
    }
    if let Ok(parsed) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(parsed, Utc));
    }
    if let Ok(parsed) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f") {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(parsed, Utc));
    }
    bail!("unsupported datetime format: {value}")
}
