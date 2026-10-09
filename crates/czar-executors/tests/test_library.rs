//! Tests for library executor
//!
//! These tests verify that the library executor can:
//! - Load dynamic libraries
//! - Call C ABI functions
//! - Handle input/output serialization
//! - Manage memory correctly
//! - Handle errors gracefully

#[cfg(feature = "library")]
mod library_tests {
    use czar_core::executor::Executor;
    use czar_executors::LibraryExecutor;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use tokio::sync::mpsc;
    use uuid::Uuid;
    fn compile_test_library(name: &str, source: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("czar-library-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let src_path = dir.join(format!("{name}.rs"));
        fs::write(&src_path, source).expect("write rust source");

        let lib_path = dir.join(format!("lib{name}.{}", std::env::consts::DLL_EXTENSION));
        let status = Command::new("rustc")
            .args([
                "--crate-type",
                "cdylib",
                "--edition",
                "2024",
                src_path.to_str().expect("source path str"),
                "-o",
                lib_path.to_str().expect("lib path str"),
            ])
            .status()
            .expect("invoke rustc");
        assert!(status.success(), "failed to compile test library");
        lib_path
    }

    fn compile_sum_tripled_library() -> PathBuf {
        compile_test_library(
            "sum_tripled",
            r#"
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

fn extract_numbers(input: &str) -> Vec<i64> {
    let mut numbers = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        if ch == '-' || ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }
        if !current.is_empty() && current != "-" {
            if let Ok(value) = current.parse::<i64>() {
                numbers.push(value);
            }
            current.clear();
        }
    }
    if !current.is_empty() && current != "-" {
        if let Ok(value) = current.parse::<i64>() {
            numbers.push(value);
        }
    }
    numbers
}

#[unsafe(no_mangle)]
pub extern "C" fn czar_task_run(input: *const c_char) -> *mut c_char {
    let input = unsafe { CStr::from_ptr(input) }.to_str().expect("utf8 input");
    let numbers = extract_numbers(input);
    let sum: i64 = numbers.iter().sum();
    let tripled = numbers
        .iter()
        .map(|n| (n * 3).to_string())
        .collect::<Vec<_>>()
        .join(",");
    let output = format!("{{\"sum\":{},\"tripled\":[{}]}}", sum, tripled);
    CString::new(output).expect("cstring").into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn czar_task_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}
"#,
        )
    }

    #[tokio::test]
    async fn test_library_executor_basic() {
        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();

        executor.set_status_channel(tx).await;

        let lib_path = compile_sum_tripled_library();
        let params = serde_json::json!({
            "library_path": lib_path.to_string_lossy(),
            "task_input": {
                "py_generate": {
                    "numbers": [1, 2, 3, 4, 5]
                }
            }
        });

        let result = executor.execute(task_id, "test_task", Some(params)).await;

        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        // Collect status updates
        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }

        // Should have received status updates
        assert!(!updates.is_empty(), "No status updates received");

        // Find the success update
        let success = updates.iter().find(|u| u.status == "success");
        assert!(success.is_some(), "No success status update found");

        let success = success.unwrap();
        assert!(success.output.is_some(), "No output in success status");

        let output = success.output.as_ref().unwrap();
        assert!(output.get("sum").is_some(), "Output missing 'sum' field");
        assert!(
            output.get("tripled").is_some(),
            "Output missing 'tripled' field"
        );

        // Verify values
        assert_eq!(output["sum"], 15); // 1+2+3+4+5 = 15
        assert_eq!(output["tripled"], serde_json::json!([3, 6, 9, 12, 15]));
    }

    #[tokio::test]
    async fn test_library_executor_missing_path() {
        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, _rx) = mpsc::unbounded_channel();

        executor.set_status_channel(tx).await;

        // No library_path provided
        let params = serde_json::json!({});

        let result = executor.execute(task_id, "test_task", Some(params)).await;

        assert!(result.is_err(), "Should fail when library_path is missing");
        assert!(
            result.unwrap_err().to_string().contains("library_path"),
            "Error should mention missing library_path"
        );
    }

    #[tokio::test]
    async fn test_library_executor_invalid_path() {
        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();

        executor.set_status_channel(tx).await;

        let params = serde_json::json!({
            "library_path": "/nonexistent/path/to/library.so"
        });

        let result = executor.execute(task_id, "test_task", Some(params)).await;

        assert!(result.is_err(), "Should fail when library doesn't exist");

        // Should have received a failed status update
        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }

        let failed = updates.iter().find(|u| u.status == "failed");
        assert!(failed.is_some(), "Should have failed status update");
        assert!(
            failed.unwrap().error.is_some(),
            "Failed update should have error message"
        );
    }

    #[tokio::test]
    async fn test_library_executor_with_empty_input() {
        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();

        executor.set_status_channel(tx).await;

        let lib_path = compile_sum_tripled_library();
        let params = serde_json::json!({
            "library_path": lib_path.to_string_lossy(),
            "task_input": {
                "py_generate": {
                    "numbers": []
                }
            }
        });

        let result = executor.execute(task_id, "test_task", Some(params)).await;

        assert!(result.is_ok(), "Should handle empty input");

        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }

        let success = updates.iter().find(|u| u.status == "success");
        assert!(success.is_some());

        let output = success.unwrap().output.as_ref().unwrap();
        assert_eq!(output["sum"], 0);
        assert_eq!(output["tripled"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn test_library_executor_status_updates() {
        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();

        executor.set_status_channel(tx).await;

        let lib_path = compile_sum_tripled_library();
        let params = serde_json::json!({
            "library_path": lib_path.to_string_lossy(),
            "task_input": {
                "py_generate": {
                    "numbers": [10, 20, 30]
                }
            }
        });

        let _result = executor.execute(task_id, "test_task", Some(params)).await;

        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }

        // Should have at least running and success updates
        assert!(updates.len() >= 2, "Should have multiple status updates");

        // First update should be running
        assert_eq!(updates[0].status, "running");
        assert_eq!(updates[0].task_id, task_id);
        assert!(updates[0].log.is_some());

        // Last update should be success
        let last = updates.last().unwrap();
        assert_eq!(last.status, "success");
        assert_eq!(last.task_id, task_id);
    }

    #[tokio::test]
    async fn test_library_executor_handles_null_pointer_output() {
        let lib_path = compile_test_library(
            "null_ptr",
            r#"
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn czar_task_run(_input: *const c_char) -> *mut c_char {
    std::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn czar_task_free(_ptr: *mut c_char) {}
"#,
        );

        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();
        executor.set_status_channel(tx).await;

        let result = executor
            .execute(
                task_id,
                "test_task",
                Some(serde_json::json!({"library_path": lib_path.to_string_lossy()})),
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("null pointer"));

        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }
        assert!(updates.iter().any(|u| u.status == "running"));
        assert!(updates.iter().any(|u| u.status == "failed"));
    }

    #[tokio::test]
    async fn test_library_executor_accepts_plain_json_output_without_prefix() {
        let lib_path = compile_test_library(
            "plain_json",
            r#"
use std::ffi::CString;
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn czar_task_run(_input: *const c_char) -> *mut c_char {
    CString::new("{\"plain\":true}").expect("cstring").into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn czar_task_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}
"#,
        );

        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();
        executor.set_status_channel(tx).await;

        let result = executor
            .execute(
                task_id,
                "test_task",
                Some(serde_json::json!({"library_path": lib_path.to_string_lossy()})),
            )
            .await;
        assert!(result.is_ok());

        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }

        let success = updates
            .iter()
            .find(|u| u.status == "success")
            .expect("success status");
        assert_eq!(success.output, Some(serde_json::json!({"plain": true})));
    }

    #[tokio::test]
    async fn test_library_executor_reports_missing_required_symbols() {
        let lib_path = compile_test_library(
            "missing_free_symbol",
            r#"
use std::ffi::CString;
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn czar_task_run(_input: *const c_char) -> *mut c_char {
    CString::new("{\"ok\":true}").expect("cstring").into_raw()
}
"#,
        );

        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();
        executor.set_status_channel(tx).await;

        let result = executor
            .execute(
                task_id,
                "test_task",
                Some(serde_json::json!({"library_path": lib_path.to_string_lossy()})),
            )
            .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to find czar_task_free")
        );
        assert!(rx.try_recv().is_ok(), "expected at least one status update");
    }

    #[tokio::test]
    async fn test_library_executor_reports_missing_run_symbol() {
        let lib_path = compile_test_library(
            "missing_run_symbol",
            r#"
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn czar_task_free(_ptr: *mut c_char) {}
"#,
        );

        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();
        executor.set_status_channel(tx).await;

        let result = executor
            .execute(
                task_id,
                "test_task",
                Some(serde_json::json!({"library_path": lib_path.to_string_lossy()})),
            )
            .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to find czar_task_run")
        );
        assert!(rx.try_recv().is_ok(), "expected at least one status update");
    }

    #[tokio::test]
    async fn test_library_executor_reports_invalid_utf8_output() {
        let lib_path = compile_test_library(
            "invalid_utf8",
            r#"
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn czar_task_run(_input: *const c_char) -> *mut c_char {
    b"\xFF\0".as_ptr() as *mut c_char
}

#[unsafe(no_mangle)]
pub extern "C" fn czar_task_free(_ptr: *mut c_char) {}
"#,
        );

        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();
        executor.set_status_channel(tx).await;

        let result = executor
            .execute(
                task_id,
                "test_task",
                Some(serde_json::json!({"library_path": lib_path.to_string_lossy()})),
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid UTF-8"));
        let updates: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(updates.iter().any(|u| u.status == "failed"));
    }
}
