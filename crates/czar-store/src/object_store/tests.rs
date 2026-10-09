use super::*;
use chrono::Utc;
use czar_core::error::CzarError;
use czar_core::types::{TaskSpec, TaskStatus, TaskStatusFile};
use czar_core::workflow::ExecutorKind;
use uuid::Uuid;

fn sample_spec(run_id: &str, task_name: &str) -> TaskSpec {
    TaskSpec {
        run_id: run_id.to_string(),
        workflow_name: "wf".to_string(),
        task_name: task_name.to_string(),
        attempt: 1,
        executor: ExecutorKind::Process,
    }
}

fn temp_store() -> (LocalObjectStore, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("czar-object-store-{}", Uuid::new_v4()));
    (LocalObjectStore::new(&dir), dir)
}

#[tokio::test]
async fn test_write_and_read_spec() {
    let (store, dir) = temp_store();
    let spec = sample_spec("run-1", "task-a");

    let path = store.write_spec(&spec).await.expect("write spec");
    assert!(path.ends_with("spec.json"));

    let loaded = store.read_spec("run-1", "task-a").await.expect("read spec");
    assert_eq!(loaded.task_name, "task-a");

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_write_and_read_status() {
    let (store, dir) = temp_store();
    let status = TaskStatusFile {
        status: TaskStatus::Running,
        started_at: Some(Utc::now()),
        finished_at: None,
        heartbeat_at: Some(Utc::now()),
        error: None,
    };

    store
        .write_status("run-1", "task-a", &status)
        .await
        .expect("write status");
    let loaded = store
        .read_status("run-1", "task-a")
        .await
        .expect("read status")
        .expect("status should exist");
    assert_eq!(loaded.status, TaskStatus::Running);

    let missing = store
        .read_status("run-1", "missing")
        .await
        .expect("read missing status");
    assert!(missing.is_none());

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_write_and_read_output() {
    let (store, dir) = temp_store();
    let output = serde_json::json!({"ok": true, "value": 42});

    store
        .write_output("run-1", "task-a", &output)
        .await
        .expect("write output");
    let loaded = store
        .read_output("run-1", "task-a")
        .await
        .expect("read output")
        .expect("output should exist");
    assert_eq!(loaded["ok"], true);
    assert_eq!(loaded["value"], 42);

    let missing = store
        .read_output("run-1", "missing")
        .await
        .expect("read missing output");
    assert!(missing.is_none());

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_object_store_path_helpers_and_missing_spec_error() {
    let (store, dir) = temp_store();
    let trait_obj: &dyn ObjectStore = &store;

    let expected_dir = dir.join("run-2").join("task-b");
    assert_eq!(trait_obj.task_dir("run-2", "task-b"), expected_dir);
    assert_eq!(
        trait_obj.spec_path("run-2", "task-b"),
        expected_dir.join("spec.json")
    );
    assert_eq!(
        trait_obj.output_path("run-2", "task-b"),
        expected_dir.join("output.json")
    );

    let err = store
        .read_spec("run-2", "missing")
        .await
        .expect_err("missing spec should error");
    assert!(matches!(err, CzarError::ReadFile { .. }));

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_read_spec_status_and_output_invalid_json_errors() {
    let (store, dir) = temp_store();
    let task_dir = store.task_dir("run-3", "task-c");
    tokio::fs::create_dir_all(&task_dir)
        .await
        .expect("create task dir");

    tokio::fs::write(store.spec_path("run-3", "task-c"), b"{not-json")
        .await
        .expect("write invalid spec");
    let spec_err = store
        .read_spec("run-3", "task-c")
        .await
        .expect_err("invalid spec json should error");
    assert!(matches!(spec_err, CzarError::JsonParse { .. }));

    tokio::fs::write(store.status_path("run-3", "task-c"), b"{bad-json")
        .await
        .expect("write invalid status");
    let status_err = store
        .read_status("run-3", "task-c")
        .await
        .expect_err("invalid status json should error");
    assert!(matches!(status_err, CzarError::JsonParse { .. }));

    tokio::fs::write(store.output_path("run-3", "task-c"), b"{also-bad")
        .await
        .expect("write invalid output");
    let output_err = store
        .read_output("run-3", "task-c")
        .await
        .expect_err("invalid output json should error");
    assert!(matches!(output_err, CzarError::JsonParse { .. }));

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_write_and_read_error_paths_return_structured_errors() {
    let (store, dir) = temp_store();
    tokio::fs::create_dir_all(&dir)
        .await
        .expect("create temp root");

    let base_file = dir.join("base-file");
    tokio::fs::write(&base_file, b"x")
        .await
        .expect("create base file");
    let bad_store = LocalObjectStore::new(&base_file);
    let spec = sample_spec("run-err", "task-err");
    let write_spec_err = bad_store
        .write_spec(&spec)
        .await
        .expect_err("create_dir_all on file path should fail");
    assert!(matches!(write_spec_err, CzarError::WriteFile { .. }));

    let status = TaskStatusFile {
        status: TaskStatus::Running,
        started_at: Some(Utc::now()),
        finished_at: None,
        heartbeat_at: Some(Utc::now()),
        error: None,
    };
    let write_status_err = bad_store
        .write_status("run-err", "task-err", &status)
        .await
        .expect_err("create_dir_all for status path on file base should fail");
    assert!(matches!(write_status_err, CzarError::WriteFile { .. }));

    let write_output_err = bad_store
        .write_output("run-err", "task-err", &serde_json::json!({"ok": true}))
        .await
        .expect_err("create_dir_all for output path on file base should fail");
    assert!(matches!(write_output_err, CzarError::WriteFile { .. }));

    let task_dir = store.task_dir("run-err", "task-err");
    tokio::fs::create_dir_all(&task_dir)
        .await
        .expect("create task dir");
    tokio::fs::create_dir(store.spec_path("run-err", "task-err"))
        .await
        .expect("create spec path dir");
    let write_spec_err = store
        .write_spec(&spec)
        .await
        .expect_err("writing spec to directory path should fail");
    assert!(matches!(write_spec_err, CzarError::WriteFile { .. }));

    tokio::fs::create_dir(store.status_path("run-err", "task-err"))
        .await
        .expect("create status path dir");
    let write_status_err = store
        .write_status("run-err", "task-err", &status)
        .await
        .expect_err("writing status to directory path should fail");
    assert!(matches!(write_status_err, CzarError::WriteFile { .. }));

    tokio::fs::create_dir(store.output_path("run-err", "task-err"))
        .await
        .expect("create output path dir");
    let write_output_err = store
        .write_output("run-err", "task-err", &serde_json::json!({"ok": true}))
        .await
        .expect_err("writing output to directory path should fail");
    assert!(matches!(write_output_err, CzarError::WriteFile { .. }));

    let read_status_err = store
        .read_status("run-err", "task-err")
        .await
        .expect_err("reading status from directory should fail");
    assert!(matches!(read_status_err, CzarError::ReadFile { .. }));

    let read_output_err = store
        .read_output("run-err", "task-err")
        .await
        .expect_err("reading output from directory should fail");
    assert!(matches!(read_output_err, CzarError::ReadFile { .. }));

    let _ = std::fs::remove_dir_all(dir);
}
