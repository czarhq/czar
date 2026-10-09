# Concepts

_Updated at: 2026-10-09_

## Examples Covered

1. [Quickstart guide example](../../examples/workflows/quickstart-guide/)
2. [Workflow schema reference](../../examples/workflows/workflow-schema-reference/)

## Workflow

A workflow is a named directed acyclic graph (DAG) of tasks, defined in YAML.

A workflow file has these top-level keys:

- `name`
- `description`
- `schedule`
- `vars`
- `types`
- `task_groups`
- `tasks`
- `flows`
- `hooks`

See [workflow-schema.md](workflow-schema.md) for the exact syntax.

## Task

A task is one unit of work inside a workflow.

Each task chooses one executor:

- `python`
- `process`
- `container`
- `cloudrun`
- `library`
- `wasm`

Tasks may also declare:

- dependencies with `depends_on`
- typed contracts with `input_type` / `output_type`
- explicit bindings with `inputs`
- retries and timeout
- conditional execution with `when`
- static or runtime fan-out with `parallel` / `foreach`

## Run

A run is one execution of a workflow definition.

Run states:

- `pending`
- `running`
- `paused`
- `success`
- `failed`
- `cancelled`

Runs can be created by:

- `czar run <workflow.yaml>`, which loads the workflow, starts a run and waits for it
- `czar trigger <workflow-name>`
- `czar run start <workflow-name>`
- the scheduler, when a deployed workflow has a `schedule`

## Task Run

A task run is one execution of one task within a run.

Task states:

- `pending`
- `dispatched`
- `running`
- `paused`
- `success`
- `failed`
- `cancelled`
- `skipped`

How a task ends up skipped or failed without running:

- A task is `skipped` when its `when:` condition is false, or when a task it depends on was skipped.
- When a task fails or is cancelled, the tasks that depend on it are marked failed.

## Flow

A flow is a named part of a workflow that you can run on its own.

A workflow describes the whole pipeline, but you often want to run only part of it: load new data without rebuilding the reports, or try the first steps while you build the rest. Instead of a second workflow file, name that part as a flow. For a workflow with the tasks `extract`, `transform` and `report`:

<!-- doc-snippet: path=examples/reference/future-workflows/06-current-flows.yaml sha256=e0e3121a1cb0f798731f6009dd8c580f856558541d7d8557a4b5b5c15cfee5d1 lines=1-3 -->
```yaml
flows:
  ingest_only:
    tasks: [extract, transform]
```

Then run only those tasks:

```bash
czar run workflow.yaml --flow ingest_only
czar trigger my_workflow --flow ingest_only
```

A flow runs the tasks it lists and every task they depend on, in the workflow's order. Written as a map (`tasks: {transform: {inputs: {...}}}`), a flow can also change a task's `inputs`, `env`, `timeout` or retries, for runs of that flow only.

## Hook

A hook is event-driven sidecar work triggered by task or run lifecycle events.

Hook events:

- `task.start`
- `task.success`
- `task.failed`
- `task.skipped`
- `run.start`
- `run.success`
- `run.failed`

Hooks use the same executors as tasks, except `container`. They are not part of the DAG: no task waits for a hook.

## Bindings

Task inputs are wired explicitly from:

- `const:` literals
- `ref:` paths

A `ref:` can point to:

- `tasks.<task>.output...`
- `vars.<NAME>`
- `parallel.index`
- `parallel.count`
- `foreach.item...`
- `foreach.index`
- `foreach.count`

Bindings are resolved when the task is about to start, after the tasks it depends on have finished.

## Variables

Workflow variables are compile-time interpolation inputs.

Override precedence:

1. workflow `vars`
2. `CZAR_VAR_<NAME>`
3. `czar run --var NAME=VALUE`

Interpolation applies to runtime string fields such as `file`, `command`, `args`, `job`, `module`, `function`, `when`, hook string fields, and string leaves inside `inputs`.

## Scheduler And State

Czar keeps all state in a database: SQLite by default, or Postgres or Cloudflare D1. The CLI, the web UI and the terminal UI all read and change that same state.

The parts:

- `czar-core` compiles and validates workflows
- `czar-executors` runs tasks
- `czar serve` runs the scheduler, the API and the web UI

The database holds:

- workflows
- workflow task rows
- workflow snapshots
- runs
- task runs
- task logs
- task outputs
- deferred jobs
- last-known workflow task state
