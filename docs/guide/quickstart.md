# Quick Start

_Updated at: 2026-05-22_

This guide shows how to add orchestration to existing code with a 2-task workflow.

## Examples Covered

1. [Quickstart guide example](../../examples/workflows/quickstart-guide/)

## Your Existing Code

You have a script that fetches data and processes it:

<!-- doc-snippet: path=examples/workflows/quickstart-guide/src/data_ops.py sha256=961d5878ad73180fb44ee90b7382ecc21401a2d7701375ed47725fc2fac0aa58 lines=1-18 -->
```py
# src/data_ops.py
import requests

def fetch_data(url: str) -> str:
    """Fetch data from URL"""
    response = requests.get(url, timeout=30)
    response.raise_for_status()
    return response.text

def summarize_data(text: str) -> dict:
    """Compute summary statistics"""
    lines = text.split('\n')

    return {
        "total_chars": len(text),
        "total_lines": len(lines),
        "non_empty_lines": len([l for l in lines if l.strip()])
    }
```

You run it like:

<!-- doc-snippet: path=examples/workflows/quickstart-guide/run.py sha256=f9d6c2c8fff17420ff88106a5e746b38cc394b8e08fa695ab14cac021f998fa1 lines=1-11 -->
```py
from src.data_ops import fetch_data, summarize_data

URL = "https://jsonplaceholder.typicode.com/posts/1"

# Fetch
data = fetch_data(URL)
print(f"Downloaded {len(data)} bytes")

# Process
stats = summarize_data(data)
print(f"Stats: {stats}")
```

**Problems**:
- Runs sequentially
- No retries on network failures
- No scheduling
- Hard to monitor which step failed

## Add Orchestration

### 1. Wrap Your Functions

Create task wrappers that expose typed task contracts and optionally receive runtime context:

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

<!-- doc-snippet: path=examples/workflows/quickstart-guide/tasks_summarize.py sha256=b87a2169db64f5e12495f460d21465921796a879ea801b7ec39de4973b045823 lines=1-18 -->
```py
# quickstart-guide/tasks_summarize.py
from czar import Context, task
from pydantic import BaseModel
from src.data_ops import summarize_data

class FetchOutput(BaseModel):
    data: str

class Output(BaseModel):
    total_chars: int
    total_lines: int
    non_empty_lines: int

@task
def main(fetch: FetchOutput, ctx: Context) -> Output:
    ctx.log("Summarizing fetched payload")
    stats = summarize_data(fetch.data)
    return Output(**stats)
```

**Key points**:
- Function parameters define the task input contract
- `Context` gives the task runtime metadata and logging hooks
- `env:` can pass static scalar environment variables into the task process
- Your existing functions (`fetch_data`, `summarize_data`) do not change
- The `summarize` task receives `fetch` output directly through its `fetch` argument
- Data passes through task outputs, not files

### 2. Define the Workflow

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

**What this does**:
- `fetch` runs first with retries on failure
- `summarize` runs after `fetch` succeeds
- `fetch.env` injects static runtime configuration into the task process
- `input_type` describes the task parameters Czar validates
- `inputs` binds constants and upstream outputs into those parameters
- `summarize` receives the whole `fetch` output via `ref: tasks.fetch.output`

### 3. What You Get

Czar now handles:
- **Dependencies** - `summarize` waits for `fetch` to complete
- **Retries** - Network failures retry automatically
- **Scheduling** - Add `schedule: "0 * * * *"` to run hourly
- **Monitoring** - See which task failed and view logs
- **Parallelization** - Scale to many URLs by adding more fetch tasks
- **Low idle cost** - Tasks that run on Cloud Run Jobs use no compute between runs

**Your business logic stayed the same**. You exposed a typed function contract and told Czar how to wire it.

## Design Principles

1. **Orchestration as afterthought** - Add orchestration without rewriting business logic
2. **Low/no cost at idle** - Designed to use as little as possible when nothing is running
3. **Parallelization included** - Run multiple fetch tasks in parallel; see [Parallelization](../design/parallelization.md)
4. **Fully open source** - No enterprise edition and no feature gating

## Next Steps

- [Tutorial: ELT Pipeline](tutorial_elt_pipeline.md) - Realistic example with medallion architecture
- [Concepts](../design/concepts.md) - Core concepts and terminology
- [Workflows](../design/workflows.md) - DAG definitions and dependency wiring
- [Tasks](../design/tasks.md) - Writing task wrappers in detail
- [Context](../design/context.md) - Runtime task metadata and helpers
- [Type Schemas](../design/type_schemas.md) - `types`, `input_type`, and `output_type`
- [Configuration](../design/configuration.md) - Retries, timeouts, vars, and scheduler tuning
- [Operations](../design/operations.md) - Deploy, run, inspect, retry, and validate
- [Deployment](../deployment/overview.md) - Local setup and securing the API
