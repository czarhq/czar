# Context

_Updated at: 2026-05-22_

<!-- Status: implemented in the Python SDK, Rust SDK helpers, and process executor runtime. -->

## Examples Covered

1. [Quickstart guide example](../../examples/workflows/quickstart-guide/)
2. [Tutorial ELT pipeline](../../examples/workflows/tutorial-elt-pipeline/)
3. [Polyglot example](../../examples/workflows/polyglot/)

## Scope

This page documents the current task `Context` surface.

Current implementations:

- Python task injection for functions that declare `ctx: Context` or `context: Context`
- Rust process-task context through `use czar::{task, Context}` or `czar::run_task_with_context(...)`

It is not part of the workflow `input_type`.

## Example

<!-- doc-snippet: path=examples/reference/future-context/04-current-usage.py sha256=9d98c6a6ace6aca73ed23f02607d01b4ee73c295a56152e02fb722e211c4701c lines=1-12 -->
```py
from czar import Context, task


@task
def main(url: str, ctx: Context) -> dict:
    ctx.log.info(f"Fetching {url}")
    ctx.metrics.record("fetch_started", 1, labels={"task": ctx.task_name})

    data = fetch(url)

    ctx.progress(1, 1, message="done")
    return {"size": len(data)}
```

## Available Fields

- `run_id`
- `workflow_name`
- `task_name`
- `task_id`
- `execution_name`
- `attempt`
- `parallel_index`
- `parallel_count`
- `raw_input`
- `upstream`

Those fields are exposed on both the Python `Context` object and the current Rust SDK `Context`.

## Logging

`ctx.log` is a lightweight task logger.

Supported calls:

- `ctx.log("message")`
- `ctx.log.info("message")`
- `ctx.log.warning("message")`
- `ctx.log.error("message")`
- `ctx.log.debug("message")`

These messages are emitted into the task log stream.

Rust example:

<!-- doc-snippet: path=examples/workflows/polyglot/rust_tasks/src/bin/process_macro.rs sha256=21bdb4b2f8c7af2485bd55a44fe34b5d7d87b36e1a31d4bce29cf9ce4f45c980 lines=24-38 -->
```rust
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
```

## Metrics

`ctx.metrics.record(name, value, labels=None)` emits a structured metric event into the task log stream.

Current behavior:

- It is useful for task-side instrumentation and debugging.
- It does not yet write to a dedicated metrics backend.

## Progress

`ctx.progress(current, total, message=None)` sends structured progress updates to the runtime.

This is the same mechanism exposed by `czar.czar_progress` in the Python SDK, but available through `Context`.

## Parallel Tasks

For Czar-managed fan-out:

- `parallel_index` is the zero-based shard index
- `parallel_count` is the total shard count

Example:

<!-- doc-snippet: path=examples/reference/future-context/05-current-parallel.py sha256=6101323b1b81a2e178e0e03062a6af691a4b0581fe74b357b270f48953d2014b lines=1-6 -->
```py
@task
def main(ctx: Context) -> dict:
    count = ctx.parallel_count or 1
    index = ctx.parallel_index or 0
    items = [item for i, item in enumerate(load_items()) if i % count == index]
    return {"count": len(items)}
```

## Notes

- `attempt` starts at `1` for the first execution attempt.
- `parallel_index` / `parallel_count` are set for static or runtime-expanded parallel tasks.
- Non-Python executors do not receive a Python `Context` object.
