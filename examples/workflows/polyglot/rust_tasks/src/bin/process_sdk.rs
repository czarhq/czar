//! Process executor with czar helpers
//!
//! This demonstrates the SDK-ASSISTED approach:
//! - Clean, minimal boilerplate
//! - Automatic stdin/stdout handling
//! - Automatic CZAR_OUTPUT prefix
//! - Task context, metrics, and progress helpers
//! - Best balance of simplicity and flexibility

use czar::{Context, run_task_with_context};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
struct TaskInput {
    rust_library: LibraryOutput,
}

#[derive(Debug, Default, Deserialize)]
struct LibraryOutput {
    sum: i32,
    tripled: Vec<i32>,
}

#[derive(Debug, Default, Serialize)]
struct TaskOutput {
    total: i32,
    tripled: Vec<i32>,
}

fn process_with_sdk(input: TaskInput, ctx: Context) -> TaskOutput {
    ctx.log().info("[Rust SDK] Processing with SDK helpers...");
    ctx.metrics()
        .record("input_sum", input.rust_library.sum, &[("task", "process_sdk")]);

    let tripled = &input.rust_library.tripled;
    let sum = input.rust_library.sum;

    ctx.log()
        .debug(&format!("[Rust SDK] Received sum: {}, tripled: {:?}", sum, tripled));

    let total = sum * 3;
    let tripled_copy = tripled.clone();

    ctx.progress(1, 1, Some("rust sdk task complete"));
    ctx.log()
        .info(&format!("[Rust SDK] Total: {}, Tripled: {:?}", total, tripled_copy));

    TaskOutput {
        total,
        tripled: tripled_copy,
    }
}

fn main() {
    // SDK handles all the boilerplate!
    run_task_with_context(process_with_sdk);
}
