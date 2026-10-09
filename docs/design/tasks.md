# Tasks

_Updated at: 2026-05-22_

<!-- Status: implemented in the Python SDK, Rust SDK helpers, runtime executors, and CLI validation. -->

## Examples Covered

1. [Quickstart guide example](../../examples/workflows/quickstart-guide/)
2. [Tutorial ELT pipeline](../../examples/workflows/tutorial-elt-pipeline/)
3. [Polyglot example](../../examples/workflows/polyglot/)
4. [Container first-class example](../../examples/workflows/container-first-class/)

## Scope

This page documents the current task authoring surface in Czar.

Today that means:

- Python tasks with `from czar import task, Context`
- Rust process tasks with `use czar::{task, Context}`
- Rust SDK helpers through `czar`
- Typed input and output contracts validated through workflow compilation
- Upstream wiring through workflow `inputs` bindings
- Non-SDK tasks through the built-in `container`, `process`, `library`, `wasm`, and `cloudrun` executors

## Minimal Form

The simplest Czar Python task has no declared types and no Pydantic; the `@task` marker is a no-op authoring tag:

<!-- doc-snippet: path=examples/workflows/quickstart-guide/tasks_minimal.py sha256=4ea5bb6520e1532fc204c2f0015227d39839da1e7d61342be9027fb3d6c096d3 lines=1-5 -->
```python
from czar import task

@task
def main():
    return {"users": 42}
```

This is enough when the workflow YAML does not declare `input_type` / `output_type` for the task. Add types only when you want them, and pick whichever style fits; the SDK does not require Pydantic specifically. Pydantic models, `TypedDict`, dataclasses, and plain `dict` returns all work.

## Python Task Shape

Python tasks are plain functions decorated with `@task`.

<!-- doc-snippet: path=examples/workflows/quickstart-guide/tasks_fetch.py sha256=279e2ffd317b648c5af22a936eda682c3b453b22e621d0222c83d761f535be9f lines=1-16 -->
```py
# quickstart-guide/tasks_fetch.py
import os

from czar import Context, task
from pydantic import BaseModel
from src.data_ops import fetch_data

class Output(BaseModel):
    data: str

@task
def main(url: str, ctx: Context) -> Output:
    source_label = os.getenv("SOURCE_LABEL", "source")
    ctx.log(f"Fetching {source_label} from {url}")
    data = fetch_data(url)
    return Output(data=data)
```

Current behavior:

- `@task` is an authoring marker. It does not change normal Python call semantics.
- Task inputs come from the workflow YAML `input_type` and `inputs` fields.
- `Context` is injected by the Python runner and is not part of the declared workflow input contract.
- Pydantic models, `TypedDict`, dataclasses, and annotated classes are introspected for schema extraction.

## Rust SDK Tasks

Rust process tasks can now use the higher-level `czar` crate and `#[task]`:

<!-- doc-snippet: path=examples/workflows/polyglot/rust_tasks/src/bin/process_macro.rs sha256=21bdb4b2f8c7af2485bd55a44fe34b5d7d87b36e1a31d4bce29cf9ce4f45c980 lines=3-45 -->
```rust
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
```

The `#[task]` macro currently targets process-style Rust tasks and generates the `main` wrapper that reads Czar input, injects `Context`, handles `czar::Result<T>`, and writes `CZAR_OUTPUT:...`.

## Rust Helper APIs

For Rust process tasks, the current helper crate is `czar`.

<!-- doc-snippet: path=examples/workflows/polyglot/rust_tasks/src/bin/process_sdk.rs sha256=7de62d717c0d8601d546c1a223ff67ea11cc03f02ebf1c640bdaff06444e1c15 lines=10-16 -->
```rust
use czar::{Context, run_task_with_context};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
struct TaskInput {
    rust_library: LibraryOutput,
}
```

<!-- doc-snippet: path=examples/workflows/polyglot/rust_tasks/src/bin/process_sdk.rs sha256=7de62d717c0d8601d546c1a223ff67ea11cc03f02ebf1c640bdaff06444e1c15 lines=30-57 -->
```rust
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
```

Current Rust helper surface:

- `#[task]` through the `czar` crate for higher-level process-task authoring
- `run_task(...)` for simple process tasks without context
- `run_task_with_context(...)` for process tasks that need runtime metadata, logging, metrics, or progress
- `czar_task_library!(...)` for native shared-library exports
- `czar_task_wasm!(...)` for wasm exports

## Context

The current Python `Context` exposes runtime metadata and lightweight task-side helpers:

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
- `log(message)`
- `log.info(...)`, `log.warning(...)`, `log.error(...)`, `log.debug(...)`
- `metrics.record(name, value, labels=None)`
- `progress(current, total, message=None)`

`parallel_index` and `parallel_count` are set for statically expanded or runtime-expanded parallel tasks. See [context.md](context.md) for the full current surface.

## Wiring Inputs

Task arguments are bound from workflow YAML.

<!-- doc-snippet: path=examples/workflows/quickstart-guide/workflow.yaml sha256=df9ef4d404c7cd0f35013adc4289ad7df1242ff23cdc68ebab0c8d8f5d37d0c1 lines=1-36 -->
```yaml
# workflows/data_pipeline.yaml
name: data_pipeline

types:
  FetchOutput:
    data: str
  SummaryOutput:
    total_chars: int
    total_lines: int
    non_empty_lines: int

tasks:
  fetch:
    executor: python
    file: tasks_fetch.py
    env:
      SOURCE_LABEL: demo-api
    input_type:
      url: str
    output_type: types.FetchOutput
    inputs:
      url:
        const: https://jsonplaceholder.typicode.com/posts/1
    timeout: 300
    retries: 3

  summarize:
    executor: python
    file: tasks_summarize.py
    depends_on: [fetch]
    input_type:
      fetch: types.FetchOutput
    output_type: types.SummaryOutput
    inputs:
      fetch:
        ref: tasks.fetch.output
```

Rules:

- `input_type` declares the named task parameters.
- `inputs` binds each parameter from either `const:` or `ref:`.
- Upstream outputs are passed explicitly through refs such as `ref: tasks.fetch.output`.
- There is no implicit `upstream` object in the workflow contract.

## Container Tasks

Container tasks are authored in workflow YAML, not through a language SDK.

Use them when you want an ordinary Czar task to execute a local Docker or Podman image while still participating in the normal DAG:

- they can depend on upstream task outputs through `inputs`
- they receive the same `CZAR_INPUT_JSON` and `CZAR_TASK_*` metadata contract as process tasks
- they emit typed outputs through `CZAR_OUTPUT:...`
- they can consume `env:` and `secrets:` just like host-side process tasks

See the runnable [container first-class example](../../examples/workflows/container-first-class/).

## Validation And Introspection

Current CLI support:

```bash
czar validate-workflow workflow.yaml
czar schema export tasks_fetch.py
```

`czar validate-workflow` compiles the workflow and checks that declared Python `input_type` and `output_type` match the function signature when a task file is available.

`czar schema export` prints the inferred contract for a Python task file:

<!-- doc-snippet: path=examples/reference/future-type-schemas/13-current-export-output.yaml sha256=9903e788eeb396c63aa1867067afeb26ff51e6eb5c8d1457102e3be9f5d993f2 lines=1-4 -->
```yaml
inputs:
  url: str
output:
  data: str
```

## Task Files And Modules

Python tasks can be loaded from either:

- `file: relative/path.py`
- `module: package.module`

Optional:

- `function: name` to override the default `main`

Project root rules:

- Python execution and introspection run through `uv run python`.
- The runtime requires a `pyproject.toml` in the resolved Python project root.
- For file-based tasks, Czar walks upward from the task file to find that project root.

## Other Task Styles

Not every task is an SDK task.

The current executor surface also supports:

- `process` for native commands and scripts
- `library` for host-native shared libraries
- `wasm` for embedded or CLI WebAssembly execution
- `cloudrun` for Cloud Run Jobs

Those task types do not use the Python `@task` decorator, but they still participate in the same workflow graph, input binding, retry, timeout, and hook machinery.
