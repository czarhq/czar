use super::*;
use chrono::Utc;

#[test]
fn test_executor_type_parse_and_from_str() {
    assert_eq!(
        ExecutorType::parse("container"),
        Some(ExecutorType::Container)
    );
    assert_eq!(
        ExecutorType::parse("cloudrun"),
        Some(ExecutorType::CloudRun)
    );
    assert_eq!(ExecutorType::parse("PROCESS"), Some(ExecutorType::Process));
    assert_eq!(ExecutorType::parse("python"), Some(ExecutorType::Python));
    assert_eq!(ExecutorType::parse("library"), Some(ExecutorType::Library));
    assert_eq!(ExecutorType::parse("wasm"), Some(ExecutorType::Wasm));
    assert_eq!(ExecutorType::parse("unknown"), None);

    assert_eq!(ExecutorType::Container.as_str(), "container");
    assert_eq!(ExecutorType::CloudRun.as_str(), "cloudrun");
    assert_eq!(ExecutorType::Process.as_str(), "process");
    assert_eq!(ExecutorType::Python.as_str(), "python");
    assert_eq!(ExecutorType::Library.as_str(), "library");
    assert_eq!(ExecutorType::Wasm.as_str(), "wasm");
    assert_eq!(
        "container".parse::<ExecutorType>(),
        Ok(ExecutorType::Container)
    );
    assert_eq!("process".parse::<ExecutorType>(), Ok(ExecutorType::Process));
    assert!("invalid".parse::<ExecutorType>().is_err());
    assert_eq!(ExecutorType::Process.to_string(), "process");
    assert_eq!(
        serde_json::to_value(ExecutorType::Library).expect("serialize executor type"),
        serde_json::json!("library")
    );
    assert_eq!(
        serde_json::from_value::<ExecutorType>(serde_json::json!("wasm"))
            .expect("deserialize executor type"),
        ExecutorType::Wasm
    );
}

#[test]
fn test_run_status_parse_and_accessors() {
    assert_eq!(RunStatus::parse("pending"), Some(RunStatus::Pending));
    assert_eq!(RunStatus::parse("RUNNING"), Some(RunStatus::Running));
    assert_eq!(RunStatus::parse("paused"), Some(RunStatus::Paused));
    assert_eq!(RunStatus::parse("success"), Some(RunStatus::Success));
    assert_eq!(RunStatus::parse("failed"), Some(RunStatus::Failed));
    assert_eq!(RunStatus::parse("other"), None);
    assert_eq!(RunStatus::Pending.as_str(), "pending");
    assert_eq!(RunStatus::Running.as_str(), "running");
    assert_eq!(RunStatus::Paused.as_str(), "paused");
    assert_eq!(RunStatus::Success.as_str(), "success");
    assert_eq!(RunStatus::Failed.as_str(), "failed");
    assert_eq!(RunStatus::Cancelled.as_str(), "cancelled");
    assert_eq!("running".parse::<RunStatus>(), Ok(RunStatus::Running));
    assert!("invalid".parse::<RunStatus>().is_err());

    let run = Run {
        id: Uuid::new_v4(),
        workflow_id: Uuid::new_v4(),
        snapshot_id: None,
        task_selection: None,
        status: RunStatus::Running,
        triggered_by: "test".to_string(),
        started_at: Some(Utc::now()),
        finished_at: None,
        error: None,
        created_at: Utc::now(),
    };
    assert_eq!(run.status(), RunStatus::Running);
    assert_eq!(run.status_str(), "running");
}

#[test]
fn test_task_and_deferred_status_accessors() {
    assert_eq!(TaskStatus::Pending.as_str(), "pending");
    assert_eq!(TaskStatus::Dispatched.as_str(), "dispatched");
    assert_eq!(TaskStatus::Running.as_str(), "running");
    assert_eq!(TaskStatus::Paused.as_str(), "paused");
    assert_eq!(TaskStatus::Success.as_str(), "success");
    assert_eq!(TaskStatus::Failed.as_str(), "failed");
    assert_eq!(TaskStatus::Cancelled.as_str(), "cancelled");

    assert_eq!(
        DeferredJobStatus::parse("completed"),
        Some(DeferredJobStatus::Completed)
    );
    assert_eq!(
        DeferredJobStatus::parse("CANCELLED"),
        Some(DeferredJobStatus::Cancelled)
    );
    assert_eq!(DeferredJobStatus::parse("bad"), None);
    assert_eq!(DeferredJobStatus::Pending.as_str(), "pending");
    assert_eq!(DeferredJobStatus::Polling.as_str(), "polling");
    assert_eq!(DeferredJobStatus::Completed.as_str(), "completed");
    assert_eq!(DeferredJobStatus::Failed.as_str(), "failed");
    assert_eq!(DeferredJobStatus::Cancelled.as_str(), "cancelled");
    assert_eq!(
        "polling".parse::<DeferredJobStatus>(),
        Ok(DeferredJobStatus::Polling)
    );
    assert!("invalid".parse::<DeferredJobStatus>().is_err());

    let deferred = DeferredJob {
        id: Uuid::new_v4(),
        task_id: Uuid::new_v4(),
        service_type: "cloud_run".to_string(),
        job_id: "job-1".to_string(),
        job_data: json_value(serde_json::json!({"k": "v"})),
        status: DeferredJobStatus::Completed,
        error: None,
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        last_polled_at: Some(Utc::now()),
        finished_at: Some(Utc::now()),
    };
    assert_eq!(deferred.status(), DeferredJobStatus::Completed);
    assert_eq!(deferred.status_str(), "completed");
}

#[test]
fn test_json_inner_returns_value_reference() {
    let value: JsonValue = json_value(serde_json::json!({"hello": "world"}));
    let inner = json_inner(&value);
    assert_eq!(inner, &serde_json::json!({"hello": "world"}));
}

#[test]
fn test_run_launch_config_supports_legacy_selection_and_inputs_envelope() {
    let legacy: JsonValue = json_value(serde_json::json!({
        "mode": "exact",
        "requested_tasks": ["extract"],
        "requested_groups": [],
        "resolved_tasks": ["extract"]
    }));
    let parsed_legacy = RunLaunchConfig::parse(Some(&legacy)).expect("parse legacy selection");
    assert_eq!(
        parsed_legacy
            .task_selection
            .as_ref()
            .expect("selection")
            .resolved_tasks,
        vec!["extract".to_string()]
    );
    assert!(parsed_legacy.inputs.is_none());

    let envelope: JsonValue = json_value(serde_json::json!({
        "task_selection": {
            "mode": "exact",
            "requested_tasks": ["extract"],
            "requested_groups": [],
            "resolved_tasks": ["extract"]
        },
        "inputs": {
            "date": "2024-01-15",
            "limit": 5
        }
    }));
    let parsed_envelope = RunLaunchConfig::parse(Some(&envelope)).expect("parse envelope");
    assert_eq!(
        parsed_envelope.inputs,
        Some(serde_json::json!({"date": "2024-01-15", "limit": 5}))
    );
}
