//! Process executor with the `#[task]` Rust authoring macro.

use czar::{Context, task};
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
    task_name: String,
}

#[task]
fn main(input: TaskInput, ctx: Context) -> czar::Result<TaskOutput> {
    let tripled = input.rust_library.tripled;
    let sum = input.rust_library.sum;
    let total = sum * 4;
    let task_name = ctx
        .task_name
        .clone()
        .unwrap_or_else(|| "rust_process_macro".to_string());

    ctx.log()
        .info(&format!("[Rust Macro] Processing sum {sum} in {task_name}"));
    ctx.metrics()
        .record("macro_input_sum", sum, &[("task", task_name.as_str())]);
    ctx.progress(1, 1, Some("rust task macro complete"));

    Ok(TaskOutput {
        total,
        tripled,
        task_name,
    })
}
