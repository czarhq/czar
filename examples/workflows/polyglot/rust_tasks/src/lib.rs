//! Shared Rust task implementations for the polyglot workflow example.
//!
//! Host builds export the native-library ABI used by `executor: library`.
//! `wasm32-unknown-unknown` builds export the embedded wasm ABI used by
//! `executor: wasm` with `wasm_mode: embedded`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
pub struct GeneratedNumbersInput {
    pub py_generate: NumbersOutput,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct NumbersOutput {
    pub numbers: Vec<i32>,
}

#[derive(Debug, Default, Serialize)]
pub struct NativeLibraryOutput {
    pub sum: i32,
    pub tripled: Vec<i32>,
}

#[derive(Debug, Default, Serialize)]
pub struct EmbeddedWasmOutput {
    pub max: i32,
    pub count: i32,
    pub squared: Vec<i32>,
}

pub fn process_via_library(input: GeneratedNumbersInput) -> NativeLibraryOutput {
    let numbers = &input.py_generate.numbers;
    let sum: i32 = numbers.iter().sum();
    let tripled: Vec<i32> = numbers.iter().map(|n| n * 3).collect();

    NativeLibraryOutput { sum, tripled }
}

pub fn process_via_wasm_embedded(input: GeneratedNumbersInput) -> EmbeddedWasmOutput {
    let numbers = &input.py_generate.numbers;
    let max = numbers.iter().copied().max().unwrap_or_default();
    let count = numbers.len() as i32;
    let squared = numbers.iter().map(|value| value * value).collect();

    EmbeddedWasmOutput {
        max,
        count,
        squared,
    }
}

#[cfg(not(target_arch = "wasm32"))]
// Export the host C ABI for the native library executor.
czar::czar_task_library!(process_via_library);

#[cfg(target_arch = "wasm32")]
// Export the embedded wasm ABI for the in-process wasm executor.
czar::czar_task_wasm!(process_via_wasm_embedded);
