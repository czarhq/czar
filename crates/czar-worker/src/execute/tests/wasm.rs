use super::*;

#[tokio::test]
async fn test_execute_handler_wasm_requires_module_path() {
    let value = run_handler(ExecuteRequest {
        executor_type: ExecutorType::Wasm.into(),
        ..Default::default()
    })
    .await;

    assert_eq!(value["status"], "failed");
    assert!(
        value["error"]
            .as_str()
            .expect("error should be string")
            .contains("Missing wasm_module_path")
    );
}

#[tokio::test]
async fn test_execute_handler_wasm_resolves_module_path_and_returns_output() {
    let base = std::env::temp_dir().join(format!("czar-worker-wasm-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");
    fs::write(base.join("workflow.yaml"), "name: wf\ntasks: {}\n").expect("write workflow");
    fs::write(base.join("task.wasm"), b"placeholder").expect("write wasm file");
    {
        use std::io::Write;

        let mut runtime = fs::File::create(base.join("fake-wasmtime.sh")).expect("write runtime");
        write!(
            runtime,
            "#!/bin/sh\nif [ \"$1\" != \"run\" ]; then exit 41; fi\nif [ \"$2\" != \"{}\" ]; then exit 42; fi\nprintf 'CZAR_OUTPUT:{{\"ok\":true}}\\n'\n",
            base.join("task.wasm").display()
        )
        .expect("write runtime");
        runtime.sync_all().expect("sync runtime");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(base.join("fake-wasmtime.sh"))
            .expect("metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(base.join("fake-wasmtime.sh"), perms).expect("chmod");
    }

    let value = run_handler_with_state(
        crate::WorkerState {
            workflow_path: base.join("workflow.yaml").to_string_lossy().to_string(),
            working_dir: base.to_string_lossy().to_string(),
        },
        ExecuteRequest {
            executor_type: ExecutorType::Wasm.into(),
            params: Some(serde_json::json!({
                "wasm_module_path": "task.wasm",
                "wasm_runtime": base.join("fake-wasmtime.sh").to_string_lossy().to_string(),
                "wasm_mode": "cli",
            })),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(value["status"], "success");
    assert_eq!(value["output"], serde_json::json!({"ok": true}));

    let _ = fs::remove_dir_all(&base);
}
