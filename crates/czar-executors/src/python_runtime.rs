//! Embedded Python runtime scripts for task execution.
//!
//! These scripts are embedded at compile time and extracted at runtime
//! to avoid external file dependencies.

/// The Python task runner script.
pub const RUN_TASK_PY: &str = include_str!("../runtime/python/run_task.py");

/// The Python custom executor runner script.
pub const RUN_EXECUTOR_PY: &str = include_str!("../runtime/python/run_executor.py");

/// The Python task introspection script.
pub const INSPECT_TASK_PY: &str = include_str!("../runtime/python/inspect_task.py");

use std::fs;
use std::io;
use std::path::PathBuf;

#[cfg(test)]
use std::sync::OnceLock;

#[cfg(test)]
use tokio::sync::Mutex;

/// Get a temporary path to the run_task.py script.
/// Creates the file if it doesn't exist.
pub fn get_run_task_script() -> io::Result<PathBuf> {
    let cache_dir = get_runtime_cache_dir()?;
    let script_path = cache_dir.join("run_task.py");

    ensure_script_content(&script_path, RUN_TASK_PY)?;

    Ok(script_path)
}

/// Get a temporary path to the run_executor.py script.
/// Creates the file if it doesn't exist.
pub fn get_run_executor_script() -> io::Result<PathBuf> {
    let cache_dir = get_runtime_cache_dir()?;
    let script_path = cache_dir.join("run_executor.py");

    ensure_script_content(&script_path, RUN_EXECUTOR_PY)?;

    Ok(script_path)
}

/// Get a temporary path to the inspect_task.py script.
/// Creates the file if it doesn't exist.
pub fn get_inspect_task_script() -> io::Result<PathBuf> {
    let cache_dir = get_runtime_cache_dir()?;
    let script_path = cache_dir.join("inspect_task.py");

    ensure_script_content(&script_path, INSPECT_TASK_PY)?;

    Ok(script_path)
}

fn ensure_script_content(path: &PathBuf, content: &str) -> io::Result<()> {
    let should_write = match fs::read_to_string(path) {
        Ok(existing) => existing != content,
        Err(_) => true,
    };
    if should_write {
        fs::write(path, content)?;
    }
    Ok(())
}

fn get_runtime_cache_dir() -> io::Result<PathBuf> {
    let cache_dir = std::env::temp_dir().join("czar").join("runtime");
    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir)
}

#[cfg(test)]
pub(crate) fn python_runtime_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
mod tests;
