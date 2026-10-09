use super::*;
use serde::Serializer;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

#[derive(Default, Deserialize)]
struct TestInput {
    value: i32,
}

#[derive(Default, Serialize)]
struct TestOutput {
    result: i32,
}

fn test_task(input: TestInput) -> TestOutput {
    TestOutput {
        result: input.value * 2,
    }
}

#[test]
fn test_task_output_trait() {
    fn accepts_output<T: TaskOutput>(_: &T) {}
    let output = test_task(TestInput { value: 21 });
    assert_eq!(output.result, 42);
    accepts_output(&output);
}

#[test]
fn test_task_input_trait() {
    fn accepts_input<T: TaskInput>(_: T) {}
    let input = TestInput { value: 21 };
    accepts_input(input);
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn restore_env_var(name: &str, previous: Option<String>) {
    match previous {
        Some(v) => unsafe { std::env::set_var(name, v) },
        None => unsafe { std::env::remove_var(name) },
    }
}

#[test]
fn test_read_input_from_upstream_env() {
    let _guard = env_lock().lock().expect("env lock");
    let previous_upstream = std::env::var("CZAR_UPSTREAM_JSON").ok();
    let previous_input = std::env::var("CZAR_INPUT_JSON").ok();

    unsafe {
        std::env::set_var("CZAR_UPSTREAM_JSON", r#"{"a":{"v":1}}"#);
        std::env::set_var("CZAR_INPUT_JSON", r#"{"value":999}"#);
    }

    #[derive(Debug, Default, Deserialize)]
    struct UpstreamInput {
        upstream: std::collections::HashMap<String, serde_json::Value>,
    }

    let parsed: UpstreamInput = read_input().expect("upstream input should parse");
    assert_eq!(parsed.upstream["a"], serde_json::json!({"v": 1}));

    restore_env_var("CZAR_UPSTREAM_JSON", previous_upstream);
    restore_env_var("CZAR_INPUT_JSON", previous_input);
}

#[test]
fn test_read_input_from_input_env() {
    let _guard = env_lock().lock().expect("env lock");
    let previous_upstream = std::env::var("CZAR_UPSTREAM_JSON").ok();
    let previous_input = std::env::var("CZAR_INPUT_JSON").ok();

    unsafe {
        std::env::remove_var("CZAR_UPSTREAM_JSON");
        std::env::set_var("CZAR_INPUT_JSON", r#"{"value":123}"#);
    }

    let parsed: TestInput = read_input().expect("input env should parse");
    assert_eq!(parsed.value, 123);

    restore_env_var("CZAR_UPSTREAM_JSON", previous_upstream);
    restore_env_var("CZAR_INPUT_JSON", previous_input);
}

#[test]
fn test_context_from_env_reads_runtime_fields() {
    let _guard = env_lock().lock().expect("env lock");
    let vars = [
        ("CZAR_RUN_ID", Some("run-123")),
        ("CZAR_WORKFLOW_NAME", Some("demo_workflow")),
        ("CZAR_TASK_NAME", Some("fanout[2]")),
        ("CZAR_TASK_ID", Some("task-456")),
        ("CZAR_EXECUTION_NAME", Some("exec-789")),
        ("CZAR_ATTEMPT", Some("3")),
        ("CZAR_PARALLEL_INDEX", None),
        ("CZAR_PARALLEL_COUNT", Some("5")),
        ("CZAR_INPUT_JSON", Some(r#"{"value":321}"#)),
        ("CZAR_UPSTREAM_JSON", Some(r#"{"fetch":{"ok":true}}"#)),
    ];
    let previous: Vec<_> = vars
        .iter()
        .map(|(name, _)| ((*name).to_string(), std::env::var(name).ok()))
        .collect();

    for (name, value) in vars {
        match value {
            Some(v) => unsafe { std::env::set_var(name, v) },
            None => unsafe { std::env::remove_var(name) },
        }
    }

    let ctx = Context::from_env();
    assert_eq!(ctx.run_id.as_deref(), Some("run-123"));
    assert_eq!(ctx.workflow_name.as_deref(), Some("demo_workflow"));
    assert_eq!(ctx.task_name.as_deref(), Some("fanout[2]"));
    assert_eq!(ctx.task_id.as_deref(), Some("task-456"));
    assert_eq!(ctx.execution_name.as_deref(), Some("exec-789"));
    assert_eq!(ctx.attempt, Some(3));
    assert_eq!(ctx.parallel_index, Some(2));
    assert_eq!(ctx.parallel_count, Some(5));
    assert_eq!(ctx.raw_input, serde_json::json!({"value": 321}));
    assert_eq!(
        ctx.upstream,
        Some(serde_json::json!({"fetch": {"ok": true}}))
    );

    for (name, value) in previous {
        restore_env_var(&name, value);
    }
}

#[test]
fn test_write_output_and_run_task_success_path() {
    let _guard = env_lock().lock().expect("env lock");
    let previous_upstream = std::env::var("CZAR_UPSTREAM_JSON").ok();
    let previous_input = std::env::var("CZAR_INPUT_JSON").ok();

    write_output(&serde_json::json!({"ok": true})).expect("write_output should succeed");

    unsafe {
        std::env::remove_var("CZAR_UPSTREAM_JSON");
        std::env::set_var("CZAR_INPUT_JSON", r#"{"value":10}"#);
    }
    run_task(|input: TestInput| TestOutput {
        result: input.value * 3,
    });

    restore_env_var("CZAR_UPSTREAM_JSON", previous_upstream);
    restore_env_var("CZAR_INPUT_JSON", previous_input);
}

#[test]
fn test_read_input_invalid_env_json_errors() {
    let _guard = env_lock().lock().expect("env lock");
    let previous_upstream = std::env::var("CZAR_UPSTREAM_JSON").ok();
    let previous_input = std::env::var("CZAR_INPUT_JSON").ok();

    unsafe {
        std::env::set_var("CZAR_UPSTREAM_JSON", "not-json");
        std::env::set_var("CZAR_INPUT_JSON", r#"{"value":123}"#);
    }

    let upstream_err: Result<TestInput, _> = read_input();
    assert!(upstream_err.is_err());

    unsafe {
        std::env::remove_var("CZAR_UPSTREAM_JSON");
        std::env::set_var("CZAR_INPUT_JSON", "{");
    }
    let input_err: Result<TestInput, _> = read_input();
    assert!(input_err.is_err());

    restore_env_var("CZAR_UPSTREAM_JSON", previous_upstream);
    restore_env_var("CZAR_INPUT_JSON", previous_input);
}

#[test]
fn test_read_input_from_stdin_fallback() {
    let _guard = env_lock().lock().expect("env lock");
    let previous_upstream = std::env::var("CZAR_UPSTREAM_JSON").ok();
    let previous_input = std::env::var("CZAR_INPUT_JSON").ok();

    unsafe {
        std::env::remove_var("CZAR_UPSTREAM_JSON");
        std::env::remove_var("CZAR_INPUT_JSON");
    }

    let mut cursor = std::io::Cursor::new(br#"{"value":456}"#.to_vec());
    let parsed: TestInput =
        read_input_from_sources(None, None, &mut cursor).expect("stdin fallback should parse");
    assert_eq!(parsed.value, 456);

    let mut invalid = std::io::Cursor::new(b"{".to_vec());
    let err: Result<TestInput, _> = read_input_from_sources(None, None, &mut invalid);
    assert!(err.is_err());

    restore_env_var("CZAR_UPSTREAM_JSON", previous_upstream);
    restore_env_var("CZAR_INPUT_JSON", previous_input);
}

#[derive(Default)]
struct FailingOutput;

impl Serialize for FailingOutput {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom("serialize failure"))
    }
}

#[test]
fn test_run_task_checked_read_and_write_error_paths() {
    let _guard = env_lock().lock().expect("env lock");
    let previous_upstream = std::env::var("CZAR_UPSTREAM_JSON").ok();
    let previous_input = std::env::var("CZAR_INPUT_JSON").ok();

    unsafe {
        std::env::set_var("CZAR_UPSTREAM_JSON", "{");
        std::env::remove_var("CZAR_INPUT_JSON");
    }
    let read_err = run_task_checked::<TestInput, TestOutput, _>(test_task);
    assert!(matches!(read_err, Err(RunTaskError::Read(_))));

    unsafe {
        std::env::remove_var("CZAR_UPSTREAM_JSON");
        std::env::set_var("CZAR_INPUT_JSON", r#"{"value":1}"#);
    }
    let write_err = run_task_checked::<TestInput, FailingOutput, _>(|_input| FailingOutput);
    assert!(matches!(write_err, Err(RunTaskError::Write(_))));

    restore_env_var("CZAR_UPSTREAM_JSON", previous_upstream);
    restore_env_var("CZAR_INPUT_JSON", previous_input);
}

#[test]
fn test_run_task_checked_with_context_success_path() {
    let _guard = env_lock().lock().expect("env lock");
    let previous_upstream = std::env::var("CZAR_UPSTREAM_JSON").ok();
    let previous_input = std::env::var("CZAR_INPUT_JSON").ok();
    let previous_task_name = std::env::var("CZAR_TASK_NAME").ok();
    let previous_attempt = std::env::var("CZAR_ATTEMPT").ok();

    unsafe {
        std::env::set_var("CZAR_INPUT_JSON", r#"{"value":9}"#);
        std::env::remove_var("CZAR_UPSTREAM_JSON");
        std::env::set_var("CZAR_TASK_NAME", "sdk_task");
        std::env::set_var("CZAR_ATTEMPT", "2");
    }

    let result = run_task_checked_with_context::<TestInput, TestOutput, _>(|input, ctx| {
        assert_eq!(ctx.task_name.as_deref(), Some("sdk_task"));
        assert_eq!(ctx.attempt, Some(2));
        TestOutput {
            result: input.value + 1,
        }
    });
    assert!(result.is_ok());

    restore_env_var("CZAR_UPSTREAM_JSON", previous_upstream);
    restore_env_var("CZAR_INPUT_JSON", previous_input);
    restore_env_var("CZAR_TASK_NAME", previous_task_name);
    restore_env_var("CZAR_ATTEMPT", previous_attempt);
}

#[derive(Default, Deserialize)]
struct FfiInput {
    value: i32,
}

#[derive(Default)]
struct FfiOutput {
    doubled: i32,
}

impl Serialize for FfiOutput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.doubled < 0 {
            return Err(serde::ser::Error::custom("negative values are unsupported"));
        }
        let mut state = serializer.serialize_struct("FfiOutput", 1)?;
        state.serialize_field("doubled", &self.doubled)?;
        state.end()
    }
}

fn ffi_task(input: FfiInput) -> FfiOutput {
    FfiOutput {
        doubled: input.value * 2,
    }
}

czar_task_library!(ffi_task);

unsafe fn ptr_to_string(ptr: *mut std::os::raw::c_char) -> String {
    let s = unsafe {
        std::ffi::CStr::from_ptr(ptr)
            .to_str()
            .expect("ffi output must be utf8")
            .to_string()
    };
    unsafe {
        czar_task_free(ptr);
    }
    s
}

#[test]
fn test_ffi_macro_handles_null_input_pointer() {
    let ptr = unsafe { czar_task_run(std::ptr::null()) };
    let output = unsafe { ptr_to_string(ptr) };
    assert!(output.contains("null input pointer"));
}

#[test]
fn test_ffi_macro_handles_invalid_utf8_and_json_parse_error() {
    let invalid_utf8 = [0xFFu8, 0u8];
    let utf8_ptr = invalid_utf8.as_ptr() as *const std::os::raw::c_char;
    let output = unsafe { ptr_to_string(czar_task_run(utf8_ptr)) };
    assert!(output.contains("invalid UTF-8"));

    let invalid_json = std::ffi::CString::new("{").expect("cstring");
    let output = unsafe { ptr_to_string(czar_task_run(invalid_json.as_ptr())) };
    assert!(output.contains("JSON parse error"));
}

#[test]
fn test_ffi_macro_success_path_prefixes_output() {
    let input = std::ffi::CString::new(r#"{"value":7}"#).expect("cstring");
    let output = unsafe { ptr_to_string(czar_task_run(input.as_ptr())) };
    assert!(output.starts_with("CZAR_OUTPUT:"));
    assert!(output.contains("\"doubled\":14"));
}

#[test]
fn test_ffi_macro_handles_json_serialize_error() {
    let input = std::ffi::CString::new(r#"{"value":-1}"#).expect("cstring");
    let output = unsafe { ptr_to_string(czar_task_run(input.as_ptr())) };
    assert!(output.contains("JSON serialize error"));
}

#[test]
fn test_restore_env_var_handles_some_and_none() {
    let _guard = env_lock().lock().expect("env lock");
    unsafe {
        std::env::set_var("CZAR_TEST_RESTORE_ENV", "before");
    }
    restore_env_var("CZAR_TEST_RESTORE_ENV", Some("after".to_string()));
    assert_eq!(
        std::env::var("CZAR_TEST_RESTORE_ENV").expect("env var"),
        "after"
    );

    restore_env_var("CZAR_TEST_RESTORE_ENV", None);
    assert!(std::env::var("CZAR_TEST_RESTORE_ENV").is_err());
}

#[test]
fn test_ffi_macro_free_accepts_null_pointer() {
    unsafe {
        czar_task_free(std::ptr::null_mut());
    }
}

#[test]
fn test_run_task_exits_with_read_error_in_subprocess() {
    if std::env::var_os("CZAR_SDK_CHILD_READ_ERROR").is_some() {
        unsafe {
            std::env::set_var("CZAR_UPSTREAM_JSON", "{");
            std::env::remove_var("CZAR_INPUT_JSON");
        }
        run_task::<TestInput, TestOutput, _>(test_task);
        panic!("run_task should have exited");
    }

    let exe = std::env::current_exe().expect("test binary path");
    let output = std::process::Command::new(exe)
        .arg("--exact")
        .arg("lib_tests::test_run_task_exits_with_read_error_in_subprocess")
        .arg("--nocapture")
        .env("CZAR_SDK_CHILD_READ_ERROR", "1")
        .output()
        .expect("run child test process");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error reading input"));
}

#[test]
fn test_run_task_exits_with_write_error_in_subprocess() {
    if std::env::var_os("CZAR_SDK_CHILD_WRITE_ERROR").is_some() {
        unsafe {
            std::env::remove_var("CZAR_UPSTREAM_JSON");
            std::env::set_var("CZAR_INPUT_JSON", r#"{"value":1}"#);
        }
        run_task::<TestInput, FailingOutput, _>(|_input| FailingOutput);
        panic!("run_task should have exited");
    }

    let exe = std::env::current_exe().expect("test binary path");
    let output = std::process::Command::new(exe)
        .arg("--exact")
        .arg("lib_tests::test_run_task_exits_with_write_error_in_subprocess")
        .arg("--nocapture")
        .env("CZAR_SDK_CHILD_WRITE_ERROR", "1")
        .output()
        .expect("run child test process");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error writing output"));
}
