//! Raw process executor (no SDK)
//!
//! This demonstrates the RAW approach:
//! - Manual env var parsing
//! - Manual JSON serialization
//! - Manual CZAR_OUTPUT prefix
//! - Maximum control, more boilerplate

use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Default, Deserialize)]
struct TaskInput {
    py_generate: NumbersOutput,
}

#[derive(Debug, Default, Deserialize)]
struct NumbersOutput {
    numbers: Vec<i32>,
}

#[derive(Debug, Default, Serialize)]
struct ProcessedOutput {
    sum: i32,
    count: i32,
}

fn main() {
    eprintln!("[Rust Raw] Starting number processing...");

    // Read task input from environment variable (manual)
    let input_json = env::var("CZAR_INPUT_JSON").expect("CZAR_INPUT_JSON not set");

    eprintln!("[Rust Raw] Received input: {}", input_json);

    let input: TaskInput = serde_json::from_str(&input_json).expect("Failed to parse input JSON");

    let numbers = &input.py_generate.numbers;
    eprintln!("[Rust Raw] Parsed {} numbers", numbers.len());

    // Process the numbers
    let sum: i32 = numbers.iter().sum();
    let count = numbers.len() as i32;

    eprintln!("[Rust Raw] Sum: {}, Count: {}", sum, count);

    let output = ProcessedOutput { sum, count };

    // Manual output with CZAR_OUTPUT prefix
    let json = serde_json::to_string(&output).expect("Failed to serialize output");
    println!("CZAR_OUTPUT:{}", json);

    eprintln!("[Rust Raw] Processing complete!");
}
