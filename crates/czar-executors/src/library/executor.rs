//! Library executor for loading and calling dynamic libraries via C ABI.
//!
//! This executor loads .so/.dll/.dylib files and calls tasks via FFI,
//! providing zero-overhead execution compared to subprocess-based executors.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use czar_core::executor::{Executor, StatusUpdate};
use libloading::{Library, Symbol};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info};
use uuid::Uuid;

/// C ABI function signature for task execution
///
/// Tasks must export: `extern "C" fn czar_task_run(input: *const c_char) -> *mut c_char`
///
/// - Input: JSON string as null-terminated C string
/// - Output: JSON string as null-terminated C string (caller must free via czar_task_free)
type TaskRunFn = unsafe extern "C" fn(*const c_char) -> *mut c_char;

/// C ABI function signature for freeing memory
///
/// Tasks must export: `extern "C" fn czar_task_free(ptr: *mut c_char)`
type TaskFreeFn = unsafe extern "C" fn(*mut c_char);

pub struct LibraryExecutor {
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
}

impl LibraryExecutor {
    pub fn new() -> Self {
        Self {
            status_tx: Arc::new(RwLock::new(None)),
        }
    }

    async fn send_status(&self, update: StatusUpdate) {
        if let Some(tx) = self.status_tx.read().await.as_ref() {
            let _ = tx.send(update);
        }
    }

    async fn execute_library(
        &self,
        task_id: Uuid,
        library_path: &Path,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        info!("Loading library: {}", library_path.display());

        // Load the dynamic library
        let lib = unsafe {
            Library::new(library_path)
                .map_err(|e| anyhow!("Failed to load library {}: {}", library_path.display(), e))?
        };

        // Determine the symbol name for the task run function.
        // Defaults to "czar_task_run" for backwards compatibility; a `function` field
        // in the task YAML can override this to enable multi-function .so files.
        let run_symbol = params
            .as_ref()
            .and_then(|p| p.get("library_function"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("czar_task_run");
        let run_symbol_c = std::ffi::CString::new(run_symbol)
            .map_err(|e| anyhow!("Invalid symbol name: {}", e))?;

        // Load the task run function
        let task_run: Symbol<TaskRunFn> = unsafe {
            lib.get(run_symbol_c.as_bytes_with_nul()).map_err(|e| {
                anyhow!(
                    "Failed to find {} function in {}: {}",
                    run_symbol,
                    library_path.display(),
                    e
                )
            })?
        };

        let task_free: Symbol<TaskFreeFn> = unsafe {
            lib.get(b"czar_task_free\0").map_err(|e| {
                anyhow!(
                    "Failed to find czar_task_free function in {}: {}",
                    library_path.display(),
                    e
                )
            })?
        };

        // Use explicitly resolved task_input only.
        let input_value = params
            .as_ref()
            .and_then(|p| p.get("task_input"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        // Prepare input JSON
        let input_json = serde_json::to_string(&input_value)?;
        let input_cstring =
            CString::new(input_json).map_err(|e| anyhow!("Invalid input JSON: {}", e))?;

        debug!(
            "Calling czar_task_run with input: {}",
            input_cstring.to_string_lossy()
        );

        // Call the task function
        let output_ptr = unsafe { task_run(input_cstring.as_ptr()) };

        if output_ptr.is_null() {
            return Err(anyhow!("Task returned null pointer"));
        }

        // Convert output to Rust string
        let output_str = unsafe {
            CStr::from_ptr(output_ptr)
                .to_str()
                .map_err(|e| anyhow!("Invalid UTF-8 in output: {}", e))?
                .to_string()
        };

        unsafe {
            task_free(output_ptr);
        }

        debug!("Task output: {}", output_str);

        // Parse output to extract CZAR_OUTPUT: prefix if present
        let result = if let Some(json_str) = output_str.strip_prefix("CZAR_OUTPUT:") {
            json_str.trim().to_string()
        } else {
            // If no prefix, treat entire output as JSON
            output_str
        };

        // Send the output as a status update
        if let Ok(output_value) = serde_json::from_str::<serde_json::Value>(&result) {
            self.send_status(StatusUpdate {
                task_id,
                status: "success".to_string(),
                log: None,
                output: Some(output_value),
                error: None,
                progress: None,
            })
            .await;
        }

        Ok(task_id.to_string())
    }
}

impl Default for LibraryExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Executor for LibraryExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        _job_name: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        // Extract library path from params
        let library_path = params
            .as_ref()
            .and_then(|p| p.get("library_path"))
            .and_then(|p| p.as_str())
            .ok_or_else(|| anyhow!("Missing library_path in task parameters"))?
            .to_string();

        let path = Path::new(&library_path);

        self.send_status(StatusUpdate {
            task_id,
            status: "running".to_string(),
            log: Some(format!("Loading library: {}\n", path.display())),
            output: None,
            error: None,
            progress: None,
        })
        .await;

        match self.execute_library(task_id, path, params).await {
            Ok(result) => Ok(result),
            Err(e) => {
                error!("Library task failed: {}", e);
                self.send_status(StatusUpdate {
                    task_id,
                    status: "failed".to_string(),
                    log: None,
                    output: None,
                    error: Some(e.to_string()),
                    progress: None,
                })
                .await;
                Err(e)
            }
        }
    }

    async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>) {
        let mut status_tx = self.status_tx.write().await;
        *status_tx = Some(tx);
    }
}
