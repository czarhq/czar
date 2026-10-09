# Workflows

_Updated at: 2026-05-22_

<!-- Status: implemented in workflow compilation, validation, and runtime execution. -->

## Examples Covered

1. [README quickstart example](../../examples/workflows/readme-quickstart/)
2. [Workflow schema reference](../../examples/workflows/workflow-schema-reference/)

## Workflow Shape

A workflow is a named DAG defined in YAML.

Current top-level fields:

- `name`
- `description`
- `schedule`
- `vars`
- `types`
- `task_groups`
- `tasks`
- `flows`
- `hooks`

See [workflow-schema.md](workflow-schema.md) for the exact grammar.

Workflows can also use project-local custom executor aliases registered in a nearby `czar.yaml`.

## Minimal Example

<!-- doc-snippet: path=examples/reference/future-workflows/04-current-minimal.yaml sha256=08ea872ddf7fde669bcf742a0ac9c921439e5702b3a4e2d254ba2f47ed4130e7 lines=1-19 -->
```yaml
name: my_etl

tasks:
  extract:
    executor: python
    file: tasks.py
    function: extract

  transform:
    executor: python
    file: tasks.py
    function: transform
    depends_on: [extract]

  load:
    executor: python
    file: tasks.py
    function: load
    depends_on: [transform]
```

Execution order:

1. `extract` runs first
2. `transform` waits for `extract`
3. `load` waits for `transform`

Tasks without dependencies can run in parallel.

## Wiring Data

Data flow is explicit.

Current model:

- declare task parameters with `input_type`
- bind them with `inputs`
- reference upstream data with `ref: tasks.<task>.output...`

Example:

<!-- doc-snippet: path=examples/reference/future-workflows/05-current-data-wiring.yaml sha256=967dd2bb8e78aca0e2730b2a1369ea5dea5c6d47fb79f34ecd7bd61a31e6b8b7 lines=1-11 -->
```yaml
tasks:
  summarize:
    executor: python
    file: tasks_summarize.py
    depends_on: [fetch]
    input_type:
      fetch:
        data: str
    inputs:
      fetch:
        ref: tasks.fetch.output
```

There is no implicit workflow-wide `upstream` object in the YAML contract.

## Types

Workflows can declare reusable type aliases under `types`.

Tasks use them through:

- `input_type`
- `output_type`

This drives compile-time validation and downstream binding checks.

## Flows

Flows are named DAG subsets for selective execution.

Example:

<!-- doc-snippet: path=examples/reference/future-workflows/06-current-flows.yaml sha256=e0e3121a1cb0f798731f6009dd8c580f856558541d7d8557a4b5b5c15cfee5d1 lines=1-3 -->
```yaml
flows:
  ingest_only:
    tasks: [extract, transform]
```

Current CLI support:

```bash
czar run workflow.yaml --flow ingest_only
czar trigger my_workflow --flow ingest_only
```

## Hooks

Hooks are lifecycle-triggered sidecar actions outside the DAG.

Current events:

- `task.start`
- `task.success`
- `task.failed`
- `task.skipped`
- `run.start`
- `run.success`
- `run.failed`

## Parallelism

Workflows can fan out tasks with:

- `parallel: <count>`
- `parallel: <upstream field>`
- `foreach: <upstream array field>`
- `parallel_mode: czar|cloud_run_tasks`

See [parallelization.md](parallelization.md) for the full behavior.
