//! Helpers and macros for writing Czar tasks compiled to WebAssembly.
//!
//! # Embedded mode (default)
//!
//! Tasks compiled to `.wasm` and run via the `wasm` executor in embedded mode must export
//! three symbols: `czar_alloc`, `czar_task_run`, and `czar_free`.  Use [`czar_task_wasm!`] to
//! generate them automatically.
//!
//! ```toml
//! # Cargo.toml
//! [lib]
//! crate-type = ["cdylib"]
//! ```
//!
//! ```rust,ignore
//! use czar_rust::czar_task_wasm;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize)]
//! struct Input { value: i32 }
//!
//! #[derive(Serialize)]
//! struct Output { doubled: i32 }
//!
//! fn my_task(input: Input) -> Output {
//!     Output { doubled: input.value * 2 }
//! }
//!
//! czar_task_wasm!(my_task);
//! ```
//!
//! # Named-symbol form
//!
//! For modules that export multiple task functions, use the two-argument form.
//! Each call exports the run function under the given name; the shared allocator
//! and free function must be exported once via [`czar_task_wasm_alloc_free!`].
//!
//! ```rust,ignore
//! czar_task_wasm!(task_a, task_a_run);
//! czar_task_wasm!(task_b, task_b_run);
//! czar_task_wasm_alloc_free!();
//! ```
//!
//! # CLI mode
//!
//! For WASI `_start` modules (wasm_mode: cli in the task YAML), use [`run_task`] from
//! the crate root — the same helper used for process executor tasks.

/// Generate the wasm embedded-mode ABI exports for a task function.
///
/// Default form exports `czar_task_run` + shared allocator + `czar_free`.
/// Named form (`czar_task_wasm!(fn, symbol)`) exports only the run symbol.
/// Call [`czar_task_wasm_alloc_free!`] once when using the named form.
#[macro_export]
macro_rules! czar_task_wasm {
    // Default form: export czar_task_run + allocator/free
    ($task_fn:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn czar_task_run(ptr: i32, len: i32) -> i64 {
            $crate::__czar_wasm_run_impl!($task_fn, ptr, len)
        }

        $crate::czar_task_wasm_alloc_free!();
    };

    // Named form: export a custom symbol; caller must invoke czar_task_wasm_alloc_free!()
    ($task_fn:ident, $symbol:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $symbol(ptr: i32, len: i32) -> i64 {
            $crate::__czar_wasm_run_impl!($task_fn, ptr, len)
        }
    };
}

/// Export the shared `czar_alloc` and `czar_free` functions for wasm embedded mode.
///
/// Included automatically by the default form of [`czar_task_wasm!`].
/// When using the named form for multi-function modules, call this once per crate.
#[macro_export]
macro_rules! czar_task_wasm_alloc_free {
    () => {
        /// Allocate `len` bytes of zeroed heap memory and return its pointer.
        /// Called by the Czar host before writing the input JSON into wasm memory.
        #[unsafe(no_mangle)]
        pub extern "C" fn czar_alloc(len: i32) -> i32 {
            let mut buf = ::std::vec::Vec::with_capacity(len as usize);
            buf.resize(len as usize, 0u8);
            let ptr = buf.as_mut_ptr() as i32;
            ::std::mem::forget(buf);
            ptr
        }

        /// Free a buffer previously returned by `czar_task_run`.
        #[unsafe(no_mangle)]
        pub extern "C" fn czar_free(ptr: i32, len: i32) {
            unsafe {
                drop(::std::vec::Vec::from_raw_parts(
                    ptr as *mut u8,
                    len as usize,
                    len as usize,
                ));
            }
        }
    };
}

/// Internal helper — do not call directly.
#[doc(hidden)]
#[macro_export]
macro_rules! __czar_wasm_run_impl {
    ($task_fn:ident, $ptr:ident, $len:ident) => {{
        let input_bytes = unsafe { ::std::slice::from_raw_parts($ptr as *const u8, $len as usize) };
        let input_str = match ::std::str::from_utf8(input_bytes) {
            Ok(s) => s,
            Err(e) => {
                let msg = format!(r#"{{"error":"invalid UTF-8 in input: {}"}}"#, e);
                let mut buf = msg.into_bytes();
                let len = buf.len() as i64;
                let ptr = buf.as_mut_ptr() as i64;
                ::std::mem::forget(buf);
                return (ptr << 32) | len;
            }
        };
        let input = match serde_json::from_str(input_str) {
            Ok(v) => v,
            Err(e) => {
                let msg = format!(r#"{{"error":"JSON parse error: {}"}}"#, e);
                let mut buf = msg.into_bytes();
                let len = buf.len() as i64;
                let ptr = buf.as_mut_ptr() as i64;
                ::std::mem::forget(buf);
                return (ptr << 32) | len;
            }
        };
        let output = $task_fn(input);
        let json = match serde_json::to_string(&output) {
            Ok(j) => format!("CZAR_OUTPUT:{}", j),
            Err(e) => format!(r#"{{"error":"JSON serialize error: {}"}}"#, e),
        };
        let mut buf = json.into_bytes();
        let out_len = buf.len() as i64;
        let out_ptr = buf.as_mut_ptr() as i64;
        ::std::mem::forget(buf);
        (out_ptr << 32) | out_len
    }};
}
