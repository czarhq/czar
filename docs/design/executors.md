# Executors

_Updated at: 2026-05-22_

<!-- Status: implemented for the built-in executor set and project-registered Python custom executors. -->

## Examples Covered

1. [Quickstart guide example](../../examples/workflows/quickstart-guide/)
2. [Container first-class example](../../examples/workflows/container-first-class/)
3. [Polyglot example](../../examples/workflows/polyglot/)
4. [Custom executor example](../../examples/workflows/custom-executor/)

## Scope

This page documents the current executor surface in Czar.

Implemented executor kinds:

- `container`
- `python`
- `process`
- `cloudrun`
- `library`
- `wasm`
- project-registered custom Python executors via `czar.yaml`

## Container

Use `executor: container` for local Docker or Podman image execution.

Required field:

- `image:`

Optional:

- `command:` to override the container entry command
- `args:` to pass structured container arguments
- `env:` and `secrets:` using the same task-side contract as `process`

Behavior:

- Czar launches the container through the local runtime found on `PATH`, preferring `docker` and then `podman`
- the workflow directory is mounted into the container at `/workspace`
- the container working directory is `/workspace`
- task input is exposed through `CZAR_INPUT_JSON`
- task output is parsed from stdout lines prefixed with `CZAR_OUTPUT:`
- mounted secret files are exposed inside the container at the declared absolute path or under `/workspace/<relative-path>`

## Python

Use `executor: python` for Python task functions.

Required fields:

- `file:` or `module:`

Optional:

- `function:` to override the default `main`

Behavior:

- Python execution runs through `uv run python`.
- File-based tasks require a discoverable `pyproject.toml` ancestor.
- `czar validate-workflow` checks declared `input_type` / `output_type` against Python annotations when a task file is available.
- `Context` is injected when a function declares `ctx: Context`.

## Process

Use `executor: process` for host commands and scripts.

Required fields:

- `command:` or `file:`

Behavior:

- path-like commands are resolved relative to the workflow file
- `args:` is passed as structured arguments
- task input is exposed through `CZAR_INPUT_JSON`
- task output is parsed from stdout lines prefixed with `CZAR_OUTPUT:`

See [native-executors.md](native-executors.md) for more detail.

## Cloud Run

Use `executor: cloudrun` for Cloud Run Jobs.

Required field:

- `job:`

Behavior:

- Czar dispatches the configured Cloud Run Job and tracks execution externally
- `parallel_mode: cloud_run_tasks` is only valid with this executor
- static or runtime fan-out can be mapped to Cloud Run `task_count` when that mode is selected

## Library

Use `executor: library` for host-native shared libraries.

Required field:

- `file:`

Optional:

- `function:` to override the default exported entrypoint

Behavior:

- the library is loaded in-process on the host
- relative paths are resolved from the workflow YAML directory

See [native-executors.md](native-executors.md) for ABI and platform details.

## Wasm

Use `executor: wasm` for WebAssembly modules.

Required field:

- `file:`

Optional:

- `function:`
- `wasm_mode: embedded|cli`

Behavior:

- `embedded` uses in-process wasmtime execution
- `cli` spawns an external runtime such as `wasmtime`

See [native-executors.md](native-executors.md) for runtime details.

## Custom Python Executors

Use a project-local `czar.yaml` to register executor aliases that point to Python classes:

<!-- doc-snippet: path=examples/workflows/custom-executor/czar.yaml sha256=dea01eeb6451a3b1fea47282cc56e5f2448e84a0663b6e166fa1d661957d1c6a lines=1-5 -->
```yaml
executors:
  sqlite_query:
    type: executors.sqlite_query:SqliteQueryExecutor
    config:
      default_limit: 10
```

Then reference that alias from workflow tasks:

<!-- doc-snippet: path=examples/reference/future-executors/09-custom-executor-task.yaml sha256=2e369962ab2617dd619ce26b2faa7918e0d787e36ab563f89c8936e1eced64e1 lines=1-4 -->
```yaml
tasks:
  query_top_customers:
    executor: sqlite_query
    config_file: queries/top_customers.json
```

Current behavior:

- the workflow loader searches the workflow directory and its ancestors for `czar.yaml`
- each custom executor alias must resolve to `module:ClassName`
- custom executor tasks must provide `config_file:`
- execution runs through the Python runtime used for ordinary Python tasks
- the executor class is instantiated with the registry `config` object from `czar.yaml`
- the runtime calls `run(...)` and passes supported parameters by name:
  - `config_file` or `config_path`
  - `ctx` or `context`
  - `upstream`
  - `task_input`, `input`, `params`, or `payload`
- hooks still use only the built-in executor set

See the runnable [custom executor example](../../examples/workflows/custom-executor/).

## Hooks Reuse The Same Executor Surface

Workflow hooks currently use the host and Cloud Run executor kinds that already have hook-specific fields in the workflow schema.

That means you can run:

- Python notification hooks
- process-based side effects
- Cloud Run hook jobs
- library or wasm hook handlers

without adding a separate hook-specific executor API.
