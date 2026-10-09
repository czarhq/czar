//! Write czar tasks in Rust.
//!
//! Mark a function with `#[czar::task]` for the process executor, or export it
//! with `czar_task_library!` (library executor) or `czar_task_wasm!` (Wasm).
//! This is the only crate task code needs: it re-exports the `#[task]` macro
//! from `czar-macros`, which Rust requires to live in a crate of its own.
//!
//! # Usage
//!
//! ## For Library Executor (Dynamic Library, High Performance)
//!
//! ```rust,ignore
//! use czar::{czar_task_library, TaskInput, TaskOutput};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize)]
//! struct MyInput {
//!     value: i32,
//! }
//!
//! #[derive(Serialize)]
//! struct MyOutput {
//!     result: i32,
//! }
//!
//! fn my_task(input: MyInput) -> MyOutput {
//!     MyOutput { result: input.value * 2 }
//! }
//!
//! // Export as C ABI for library executor
//! czar_task_library!(my_task);
//! ```
//!
//! Build as `cdylib`:
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//! ```
//!
//! ## For Process Executor (Binary, Maximum Compatibility)
//!
//! ```rust,ignore
//! use czar::run_task;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize)]
//! struct MyInput {
//!     value: i32,
//! }
//!
//! #[derive(Serialize)]
//! struct MyOutput {
//!     result: i32,
//! }
//!
//! fn my_task(input: MyInput) -> MyOutput {
//!     MyOutput { result: input.value * 2 }
//! }
//!
//! fn main() {
//!     run_task(my_task);
//! }
//! ```
//!
//! ## For Process Executor With Task Context
//!
//! ```rust,ignore
//! use czar::{Context, run_task_with_context};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize)]
//! struct MyInput {
//!     value: i32,
//! }
//!
//! #[derive(Serialize)]
//! struct MyOutput {
//!     result: i32,
//! }
//!
//! fn my_task(input: MyInput, ctx: Context) -> MyOutput {
//!     ctx.log().info("processing input");
//!     ctx.metrics().record("input_value", input.value, &[("task", "my_task")]);
//!     MyOutput { result: input.value * 2 }
//! }
//!
//! fn main() {
//!     run_task_with_context(my_task);
//! }
//! ```

pub mod deferrables;
pub mod wasm;

pub use czar_macros::task;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::BufRead;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Context {
    pub run_id: Option<String>,
    pub workflow_name: Option<String>,
    pub task_name: Option<String>,
    pub task_id: Option<String>,
    pub execution_name: Option<String>,
    pub attempt: Option<i32>,
    pub parallel_index: Option<i32>,
    pub parallel_count: Option<i32>,
    pub raw_input: Value,
    pub upstream: Option<Value>,
}

impl Context {
    pub fn from_env() -> Self {
        let task_name = std::env::var("CZAR_TASK_NAME").ok();
        Self {
            run_id: std::env::var("CZAR_RUN_ID").ok(),
            workflow_name: std::env::var("CZAR_WORKFLOW_NAME").ok(),
            task_name: task_name.clone(),
            task_id: std::env::var("CZAR_TASK_ID").ok(),
            execution_name: std::env::var("CZAR_EXECUTION_NAME").ok(),
            attempt: env_int("CZAR_ATTEMPT"),
            parallel_index: env_int("CZAR_PARALLEL_INDEX")
                .or_else(|| task_name.as_deref().and_then(parallel_index_from_task_name)),
            parallel_count: env_int("CZAR_PARALLEL_COUNT"),
            raw_input: json_env("CZAR_INPUT_JSON").unwrap_or_else(|| serde_json::json!({})),
            upstream: json_env("CZAR_UPSTREAM_JSON"),
        }
    }

    pub fn log(&self) -> ContextLogger<'_> {
        ContextLogger { _context: self }
    }

    pub fn metrics(&self) -> ContextMetrics<'_> {
        ContextMetrics { _context: self }
    }

    pub fn progress(&self, current: usize, total: usize, message: Option<&str>) {
        let mut payload = serde_json::Map::new();
        payload.insert("current".to_string(), serde_json::json!(current));
        payload.insert("total".to_string(), serde_json::json!(total));
        if let Some(message) = message {
            payload.insert("message".to_string(), serde_json::json!(message));
        }
        println!("CZAR_PROGRESS:{}", Value::Object(payload));
    }
}

pub struct ContextLogger<'a> {
    _context: &'a Context,
}

impl ContextLogger<'_> {
    pub fn info(&self, message: &str) {
        emit_log("INFO", message);
    }

    pub fn warning(&self, message: &str) {
        emit_log("WARNING", message);
    }

    pub fn error(&self, message: &str) {
        emit_log("ERROR", message);
    }

    pub fn debug(&self, message: &str) {
        emit_log("DEBUG", message);
    }
}

pub struct ContextMetrics<'a> {
    _context: &'a Context,
}

impl<'a> Default for ContextLogger<'a> {
    fn default() -> Self {
        let context = Box::leak(Box::new(Context::default()));
        Self { _context: context }
    }
}

impl<'a> Default for ContextMetrics<'a> {
    fn default() -> Self {
        let context = Box::leak(Box::new(Context::default()));
        Self { _context: context }
    }
}

impl ContextMetrics<'_> {
    pub fn record<V>(&self, name: &str, value: V, labels: &[(&str, &str)])
    where
        V: Serialize,
    {
        let mut payload = serde_json::Map::new();
        payload.insert("name".to_string(), serde_json::json!(name));
        payload.insert(
            "value".to_string(),
            serde_json::to_value(value).unwrap_or(Value::Null),
        );
        if !labels.is_empty() {
            let labels_obj = labels
                .iter()
                .map(|(key, value)| (key.to_string(), serde_json::json!(value)))
                .collect();
            payload.insert("labels".to_string(), Value::Object(labels_obj));
        }
        println!("CZAR_METRIC:{}", Value::Object(payload));
    }
}

fn env_int(name: &str) -> Option<i32> {
    std::env::var(name).ok()?.parse().ok()
}

fn json_env(name: &str) -> Option<Value> {
    let raw = std::env::var(name).ok()?;
    serde_json::from_str(&raw).ok()
}

fn parallel_index_from_task_name(task_name: &str) -> Option<i32> {
    let suffix = task_name.strip_suffix(']')?;
    let index_start = suffix.rfind('[')?;
    suffix[index_start + 1..].parse().ok()
}

fn emit_log(level: &str, message: &str) {
    match level {
        "INFO" => println!("{message}"),
        _ => println!("{level}: {message}"),
    }
}

/// Read task input from environment variables or stdin
///
/// For process executor with upstream: reads from CZAR_UPSTREAM_JSON env var
/// For process executor with task_input: reads from CZAR_INPUT_JSON env var
/// Fallback: reads from stdin
pub fn read_input<T: for<'de> Deserialize<'de>>() -> Result<T, Box<dyn std::error::Error>> {
    let upstream_json = std::env::var("CZAR_UPSTREAM_JSON").ok();
    let input_json = std::env::var("CZAR_INPUT_JSON").ok();
    let stdin = std::io::stdin();
    let mut stdin_lock = stdin.lock();
    read_input_from_sources(
        upstream_json.as_deref(),
        input_json.as_deref(),
        &mut stdin_lock,
    )
}

fn read_input_from_sources<T: for<'de> Deserialize<'de>, R: BufRead>(
    upstream_json: Option<&str>,
    input_json: Option<&str>,
    stdin: &mut R,
) -> Result<T, Box<dyn std::error::Error>> {
    // Try CZAR_UPSTREAM_JSON first (for tasks with dependencies)
    if let Some(upstream_json) = upstream_json {
        let value: serde_json::Value = serde_json::from_str(upstream_json)?;
        // Wrap in task_input structure
        let wrapped = serde_json::json!({"upstream": value});
        return Ok(serde_json::from_value(wrapped)?);
    }

    // Try CZAR_INPUT_JSON (for tasks with explicit input)
    if let Some(input_json) = input_json {
        return Ok(serde_json::from_str(input_json)?);
    }

    // Fallback to stdin
    let mut input = String::new();
    stdin.read_line(&mut input)?;
    let parsed = serde_json::from_str(&input)?;
    Ok(parsed)
}

/// Write task output to stdout with CZAR_OUTPUT: prefix
pub fn write_output<T: Serialize>(output: &T) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(output)?;
    println!("CZAR_OUTPUT:{}", json);
    Ok(())
}

/// Run a task function with input from stdin and output to stdout
///
/// This is the main entry point for process executor tasks.
///
/// # Example
///
/// ```rust,ignore
/// fn main() {
///     run_task(my_task_function);
/// }
/// ```
pub fn run_task<I, O, F>(task_fn: F)
where
    I: for<'de> Deserialize<'de>,
    O: Serialize,
    F: FnOnce(I) -> O,
{
    match run_task_checked(task_fn) {
        Ok(()) => {}
        Err(RunTaskError::Read(e)) => {
            eprintln!("Error reading input: {}", e);
            std::process::exit(1);
        }
        Err(RunTaskError::Write(e)) => {
            eprintln!("Error writing output: {}", e);
            std::process::exit(1);
        }
    }
}

/// Run a task function with input from stdin/env and a populated Czar context.
pub fn run_task_with_context<I, O, F>(task_fn: F)
where
    I: for<'de> Deserialize<'de>,
    O: Serialize,
    F: FnOnce(I, Context) -> O,
{
    match run_task_checked_with_context(task_fn) {
        Ok(()) => {}
        Err(RunTaskError::Read(e)) => {
            eprintln!("Error reading input: {}", e);
            std::process::exit(1);
        }
        Err(RunTaskError::Write(e)) => {
            eprintln!("Error writing output: {}", e);
            std::process::exit(1);
        }
    }
}

#[derive(Debug)]
enum RunTaskError {
    Read(Box<dyn std::error::Error>),
    Write(Box<dyn std::error::Error>),
}

fn run_task_checked<I, O, F>(task_fn: F) -> Result<(), RunTaskError>
where
    I: for<'de> Deserialize<'de>,
    O: Serialize,
    F: FnOnce(I) -> O,
{
    let input = read_input::<I>().map_err(RunTaskError::Read)?;
    let output = task_fn(input);
    write_output(&output).map_err(RunTaskError::Write)?;
    Ok(())
}

fn run_task_checked_with_context<I, O, F>(task_fn: F) -> Result<(), RunTaskError>
where
    I: for<'de> Deserialize<'de>,
    O: Serialize,
    F: FnOnce(I, Context) -> O,
{
    let input = read_input::<I>().map_err(RunTaskError::Read)?;
    let context = Context::from_env();
    let output = task_fn(input, context);
    write_output(&output).map_err(RunTaskError::Write)?;
    Ok(())
}

/// Helper trait for task inputs
pub trait TaskInput: for<'de> Deserialize<'de> {}
impl<T: for<'de> Deserialize<'de>> TaskInput for T {}

/// Helper trait for task outputs
pub trait TaskOutput: Serialize {}
impl<T: Serialize> TaskOutput for T {}

/// Generate C ABI exports for library executor.
///
/// # Default form — exports `czar_task_run` + `czar_task_free`
///
/// Use this when the `.so` contains a single task function:
///
/// ```rust,ignore
/// fn process_numbers(input: NumbersInput) -> ProcessedOutput { ... }
/// czar_task_library!(process_numbers);
/// ```
///
/// # Named form — exports a custom symbol name
///
/// Use this when the `.so` contains multiple task functions.
/// The corresponding YAML task must set `function: <symbol>` to select it.
/// Call this once per function; `czar_task_free` must be exported exactly once,
/// either via one default-form invocation or separately.
///
/// ```rust,ignore
/// fn task_a(input: InputA) -> OutputA { ... }
/// fn task_b(input: InputB) -> OutputB { ... }
///
/// czar_task_library!(task_a, task_a_run);
/// czar_task_library!(task_b, task_b_run);
///
/// // Export the shared free function exactly once:
/// czar_task_library_free!();
/// ```
#[macro_export]
macro_rules! czar_task_library {
    // Default form: exports czar_task_run + czar_task_free
    ($task_fn:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn czar_task_run(
            input_ptr: *const ::std::os::raw::c_char,
        ) -> *mut ::std::os::raw::c_char {
            $crate::__czar_library_run_impl!($task_fn, input_ptr)
        }

        $crate::czar_task_library_free!();
    };

    // Named form: exports a custom symbol; czar_task_free must be provided separately
    ($task_fn:ident, $symbol:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $symbol(
            input_ptr: *const ::std::os::raw::c_char,
        ) -> *mut ::std::os::raw::c_char {
            $crate::__czar_library_run_impl!($task_fn, input_ptr)
        }
    };
}

/// Export the shared `czar_task_free` function.
///
/// This is included automatically by the default form of [`czar_task_library!`].
/// When using the named form for multi-function `.so` files, call this macro
/// exactly once per crate to export the free function.
#[macro_export]
macro_rules! czar_task_library_free {
    () => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn czar_task_free(ptr: *mut ::std::os::raw::c_char) {
            if !ptr.is_null() {
                unsafe {
                    let _ = ::std::ffi::CString::from_raw(ptr);
                }
            }
        }
    };
}

/// Internal helper — do not call directly.
#[doc(hidden)]
#[macro_export]
macro_rules! __czar_library_run_impl {
    ($task_fn:ident, $input_ptr:ident) => {{
        use ::std::ffi::{CStr, CString};

        if $input_ptr.is_null() {
            let error = CString::new(r#"{"error": "null input pointer"}"#).unwrap();
            return error.into_raw();
        }

        let input_str = unsafe {
            match CStr::from_ptr($input_ptr).to_str() {
                Ok(s) => s,
                Err(e) => {
                    let error =
                        CString::new(format!(r#"{{"error": "invalid UTF-8: {}"}}"#, e)).unwrap();
                    return error.into_raw();
                }
            }
        };

        let input = match serde_json::from_str(input_str) {
            Ok(i) => i,
            Err(e) => {
                let error =
                    CString::new(format!(r#"{{"error": "JSON parse error: {}"}}"#, e)).unwrap();
                return error.into_raw();
            }
        };

        let output = $task_fn(input);

        let output_json = match serde_json::to_string(&output) {
            Ok(j) => format!("CZAR_OUTPUT:{}", j),
            Err(e) => {
                let error = format!(r#"{{"error": "JSON serialize error: {}"}}"#, e);
                return CString::new(error).unwrap().into_raw();
            }
        };

        CString::new(output_json).unwrap().into_raw()
    }};
}

#[cfg(test)]
mod lib_tests;

pub type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

pub trait IntoTaskResult<T> {
    type Error: std::fmt::Display;

    fn into_task_result(self) -> std::result::Result<T, Self::Error>;
}

impl<T> IntoTaskResult<T> for T {
    type Error = std::convert::Infallible;

    fn into_task_result(self) -> std::result::Result<T, Self::Error> {
        Ok(self)
    }
}

impl<T, E> IntoTaskResult<T> for std::result::Result<T, E>
where
    E: std::fmt::Display,
{
    type Error = E;

    fn into_task_result(self) -> std::result::Result<T, Self::Error> {
        self
    }
}

pub fn into_task_result<T, R>(value: R) -> std::result::Result<T, R::Error>
where
    R: IntoTaskResult<T>,
{
    value.into_task_result()
}
