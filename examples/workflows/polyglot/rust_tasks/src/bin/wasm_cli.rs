//! WASI/WebAssembly task example for `wasm_mode: cli`.
//!
//! This demonstrates the subprocess-style wasm path:
//! - compile Rust to a WASI `.wasm` binary
//! - let Czar run it through a wasm runtime such as `wasmtime run`
//! - use the same CZAR input/output JSON contract as `executor: process`

use czar::run_task;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
struct TaskInput {
    py_generate: NumbersOutput,
}

#[derive(Debug, Default, Deserialize)]
struct NumbersOutput {
    numbers: Vec<i32>,
}

#[derive(Debug, Default, Serialize)]
struct TaskOutput {
    min: i32,
    count: i32,
    even_numbers: Vec<i32>,
}

fn process_with_wasm_cli(input: TaskInput) -> TaskOutput {
    let numbers = &input.py_generate.numbers;

    TaskOutput {
        min: numbers.iter().copied().min().unwrap_or_default(),
        count: numbers.len() as i32,
        even_numbers: numbers
            .iter()
            .copied()
            .filter(|value| value % 2 == 0)
            .collect(),
    }
}

fn main() {
    run_task(process_with_wasm_cli);
}
