#[cfg(feature = "process")]
pub mod process;

#[cfg(feature = "process")]
pub mod python;

#[cfg(feature = "cloudrun")]
pub mod cloud_run;

#[cfg(any(feature = "process", feature = "cloudrun"))]
mod env;

#[cfg(feature = "library")]
pub mod library;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "worker")]
pub mod worker;

pub mod dispatch;
pub mod manager;
pub mod python_runtime;

#[cfg(feature = "process")]
pub use process::ProcessExecutor;

#[cfg(feature = "process")]
pub use python::PythonExecutor;

#[cfg(feature = "cloudrun")]
pub use cloud_run::CloudRunClient;

#[cfg(feature = "library")]
pub use library::LibraryExecutor;

#[cfg(feature = "wasm")]
pub use wasm::WasmExecutor;

#[cfg(feature = "worker")]
pub use worker::WorkerExecutor;

pub use manager::ExecutorManager;

// Re-export the Executor trait from czar-core
pub use czar_core::executor::{Executor, StatusUpdate};
