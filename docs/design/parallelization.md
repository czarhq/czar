# Parallelization

_Updated at: 2026-05-22_

<!-- Status: implemented in core runtime, workflow validation, and Cloud Run executor integration. -->

## Examples Covered

1. [Parallelization reference](../../examples/reference/future-parallelization/)

Czar supports three parallelization patterns today: static compile-time expansion, runtime fan-out from upstream task outputs (`parallel: <field>` / `foreach: <array field>`), and Cloud Run native task-count execution.

For `foreach`, the upstream field must be declared as an array type. Accepted array syntax includes `[str]`, `list[str]`, `List[str]`, `array[str]`, `Array[str]`, and `str[]`.

## Static Parallelization (`parallel_mode: czar`)

Run a task as a fixed number of independent instances. The count is determined at compile time and each instance becomes a separate Czar task record.

<!-- doc-snippet: path=examples/reference/future-parallelization/01-static-parallel.yaml sha256=770635aa99becdb85968f8aca63eb2a4293591e00d9c9e15ad7d705de73e2bf4 lines=1-5 -->
```yaml
tasks:
  process:
    executor: python
    file: tasks/process.py
    parallel: 4
```

This compiles into four task records: `process[0]`, `process[1]`, `process[2]`, `process[3]`. Each receives the following extra parameters at execution time:

- `parallel_index`: zero-based index of this instance
- `parallel_count`: total number of instances

### Task Implementation

<!-- doc-snippet: path=examples/reference/future-parallelization/02-task-implementation.py sha256=154710dcd4c6f0619aae36e4b32022ca668749015dfcc3e04d5e48951c3edfa5 lines=1-11 -->
```python
# tasks/process.py

def main(input, ctx):
    # Divide work by index
    all_items = get_all_items()
    my_items = [
        item for i, item in enumerate(all_items)
        if i % ctx.parallel_count == ctx.parallel_index
    ]
    results = process_items(my_items)
    return {"processed": len(results)}
```

### Downstream Dependencies

A task that depends on a statically-expanded task automatically depends on **all** instances. You continue to reference the base name in `depends_on` and in `ref` bindings; Czar rewrites the dependency list at compile time.

<!-- doc-snippet: path=examples/reference/future-parallelization/03-downstream-dependencies.yaml sha256=e9377d02bfcb38fa5b77ee036083929617b1cea39398c4646188e394bff9e4e4 lines=1-17 -->
```yaml
tasks:
  process:
    executor: python
    file: tasks/process.py
    parallel: 4
    output_type:
      processed: int

  aggregate:
    executor: python
    file: tasks/aggregate.py
    depends_on: [process]
    input_type:
      process: [{processed: int}]
    inputs:
      process:
        ref: tasks.process.output
```

The `ref: tasks.process.output` binding resolves to a JSON array containing the output of each instance in index order: `[{processed: 12}, {processed: 9}, ...]`.

You can also extract a specific field across all instances:

<!-- doc-snippet: path=examples/reference/future-parallelization/04-field-extraction.yaml sha256=281b96f820c85c804a25ccbd4d8fe9e03d87f14a9c36c5f375827792edca2bec lines=1-3 -->
```yaml
inputs:
  counts:
    ref: tasks.process.output.processed  # → [12, 9, ...]
```

### Concurrency Limit

Limit how many instances run at the same time:

<!-- doc-snippet: path=examples/reference/future-parallelization/05-concurrency.yaml sha256=f6432bdc7ac8f036eaabd96d7b93c4071df6a9f9a43af52b92c1ebc57b657ea1 lines=1-6 -->
```yaml
tasks:
  process:
    executor: python
    file: tasks/process.py
    parallel: 100
    concurrency: 8  # at most 8 run simultaneously
```

The scheduler now enforces `concurrency` for `parallel_mode: czar` fan-outs in the local/runtime path.

Current behavior:

- at most `concurrency` instances from the same `parallel` / `foreach` group are dispatched at once
- `dispatched` and `running` instances count against the limit
- once an active instance finishes, the next pending instance in that group becomes eligible on the next scheduler pass
- Cloud Run native `parallel_mode: cloud_run_tasks` still delegates concurrency to Cloud Run itself

## Cloud Run Native Parallelism (`parallel_mode: cloud_run_tasks`)

For Cloud Run Jobs, Czar can delegate instance management to Cloud Run's native `task_count` feature. This emits a single Czar task record and passes `task_count` directly to the Cloud Run API, keeping Czar's metadata overhead constant regardless of fan-out size.

<!-- doc-snippet: path=examples/reference/future-parallelization/06-cloud-run-native.yaml sha256=8f3f79386009fdb98303c72533f24ea5401225a80b57e00c7f42429929d5f565 lines=1-6 -->
```yaml
tasks:
  process:
    executor: cloudrun
    job: my-process-job
    parallel: 1000
    parallel_mode: cloud_run_tasks
```

Cloud Run will execute the job with 1000 task instances, providing the standard `CLOUD_RUN_TASK_INDEX` and `CLOUD_RUN_TASK_COUNT` environment variables to each instance.

This mode is ideal for large fan-outs where tracking thousands of individual Czar task records would be unnecessary overhead.

## Dynamic Parallelization

Czar also supports runtime fan-out from upstream task outputs.

<!-- doc-snippet: path=examples/reference/future-parallelization/07-dynamic-parallelization.yaml sha256=721cd8193259abe37a90a6dcf44aab9f57d76499e72bb25045355c237df79ba6 lines=1-36 -->
```yaml
# Count from upstream output
tasks:
  discover:
    executor: python
    file: tasks/discover.py
    output_type:
      count: int
      files: list[str]

  process:
    executor: python
    file: tasks/process.py
    depends_on: [discover]
    parallel: discover.count

  aggregate:
    executor: python
    file: tasks/aggregate.py
    depends_on: [process]
    input_type:
      slots: [int]
    inputs:
      slots:
        ref: tasks.process.output.slot

# Foreach
  process_file:
    executor: python
    file: tasks/process_file.py
    depends_on: [discover]
    foreach: discover.files
    input_type:
      file: str
    inputs:
      file:
        ref: foreach.item
```

In practice, `foreach: discover.files` validates only when `discover.output_type.files` is declared as an array field.

For `parallel_mode: czar`, the scheduler materializes concrete task instances at runtime:
- `parallel: discover.count` becomes `task[0]`, `task[1]`, ...
- `foreach: discover.files` becomes one instance per array item
- downstream `depends_on: [task]` entries are rewritten to depend on all materialized instances
- downstream `ref: tasks.task.output...` bindings aggregate instance outputs into arrays in index order

Runtime task inputs can also bind fan-out context directly:
- `parallel.index`
- `parallel.count`
- `foreach.item`
- `foreach.index`
- `foreach.count`

`parallel_mode: cloud_run_tasks` also supports runtime `parallel: <upstream field>` by resolving the count before dispatch and passing it to Cloud Run as `task_count`. `foreach` is not valid with `cloud_run_tasks` because Cloud Run native task counts cannot carry distinct per-item payloads.

## Use Cases

### Static Parallelization
- Known partition count (e.g., process data for 12 months)
- Fixed shard count for distributed processing
- Small-to-medium fan-outs where per-instance tracking is useful

### Cloud Run Native
- Large fan-outs (hundreds to thousands of instances)
- Cases where Cloud Run's built-in retry and scheduling is preferable
- Minimising Czar scheduler overhead for throughput-sensitive pipelines
