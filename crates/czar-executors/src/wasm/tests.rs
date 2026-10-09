use std::fs;
use std::path::{Path, PathBuf};

use tokio::sync::mpsc;
use uuid::Uuid;

use czar_core::executor::Executor;

use super::WasmExecutor;
use super::support::is_wasmtime_runtime;

fn write_runtime_script(
    dir: &Path,
    module_path: &Path,
    should_fail: bool,
) -> std::io::Result<PathBuf> {
    let script_path = dir.join("fake-wasmtime.sh");
    let body = format!(
        "#!/bin/sh\nif [ \"$1\" != \"run\" ]; then echo bad-argv >&2; exit 91; fi\nif [ \"$2\" != \"{module}\" ]; then echo bad-module >&2; exit 92; fi\nif [ \"$CZAR_INPUT_JSON\" != '{{\"name\":\"world\"}}' ]; then echo bad-input >&2; exit 93; fi\nif [ \"$CZAR_WASM_MODULE\" != \"{module}\" ]; then echo bad-env >&2; exit 94; fi\nif [ {should_fail} -eq 1 ]; then echo runtime-failed >&2; exit 7; fi\nprintf 'booting wasm\\n'\nprintf 'CZAR_OUTPUT:{{\"ok\":true}}\\n'\n",
        module = module_path.display(),
        should_fail = if should_fail { 1 } else { 0 }
    );
    fs::write(&script_path, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)?;
    }
    Ok(script_path)
}

#[tokio::test]
async fn execute_wasm_module_reports_success() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let module_path = temp_dir.path().join("task.wasm");
    fs::write(&module_path, b"placeholder").expect("write wasm placeholder");
    let runtime = write_runtime_script(temp_dir.path(), &module_path, false).expect("script");

    let executor = WasmExecutor::new(Some(temp_dir.path().to_string_lossy().to_string()));
    let task_id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;

    let result = executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "wasm_module_path": module_path.to_string_lossy().to_string(),
                "wasm_runtime": runtime.to_string_lossy().to_string(),
                "wasm_mode": "cli",
                "task_input": {"name": "world"},
            })),
        )
        .await;

    assert!(result.is_ok(), "wasm execution should dispatch");

    let mut updates = Vec::new();
    while let Some(update) = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for update")
    {
        let terminal = update.status == "success" || update.status == "failed";
        updates.push(update);
        if terminal {
            break;
        }
    }

    assert!(updates.iter().any(|u| u.status == "running"));
    let success = updates
        .iter()
        .find(|u| u.status == "success")
        .expect("success update");
    assert_eq!(success.output, Some(serde_json::json!({"ok": true})));
}

#[tokio::test]
async fn execute_wasm_module_requires_module_path() {
    let executor = WasmExecutor::default();
    let err = executor
        .execute(Uuid::new_v4(), "ignored", Some(serde_json::json!({})))
        .await
        .expect_err("missing module path should fail");
    assert!(err.to_string().contains("wasm_module_path"));
}

#[tokio::test]
async fn execute_wasm_module_reports_runtime_failure() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let module_path = temp_dir.path().join("task.wasm");
    fs::write(&module_path, b"placeholder").expect("write wasm placeholder");
    let runtime = write_runtime_script(temp_dir.path(), &module_path, true).expect("script");

    let executor = WasmExecutor::new(Some(temp_dir.path().to_string_lossy().to_string()));
    let task_id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;

    executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "wasm_module_path": module_path.to_string_lossy().to_string(),
                "wasm_runtime": runtime.to_string_lossy().to_string(),
                "wasm_mode": "cli",
                "task_input": {"name": "world"},
            })),
        )
        .await
        .expect("dispatch should succeed");

    let mut failure = None;
    while let Some(update) = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for update")
    {
        if update.status == "failed" {
            failure = Some(update);
            break;
        }
    }

    let failure = failure.expect("failed update");
    assert!(
        !failure.error.as_deref().unwrap_or_default().is_empty(),
        "failed update should include an error message"
    );
}

#[test]
fn detects_wasmtime_runtime_name() {
    assert!(is_wasmtime_runtime("wasmtime"));
    assert!(is_wasmtime_runtime("/usr/local/bin/wasmtime"));
    assert!(!is_wasmtime_runtime("wasmer"));
}
