# Workflow Schema

_Updated at: 2026-05-22_

<!-- Status: implemented in core runtime and CLI validation. -->

## Examples Covered

1. [Workflow schema reference](../../examples/workflows/workflow-schema-reference/)

## Scope
This document defines the current workflow schema and validation rules implemented by the Czar runtime.

CLI support:
- Validate command: `czar validate-workflow <file.yaml>`
- `czar validate-workflow` uses the same workflow-definition parser and validator as runtime workflow loading.
- Validation rejects legacy v1-style `upstream` conventions.

## Design goals
- No implicit `upstream` injection.
- No untyped/bare input values in task bindings.
- Explicit task contracts when inputs or outputs are declared.
- Deterministic, cross-language validation behavior.

## Top-level schema
<!-- doc-snippet: path=examples/workflows/workflow-schema-reference/top-level-schema.yaml sha256=2f7d919b9ae9a250161e90c513b69bcf53a3fb13d4c0cf0f02e0582be624d19d lines=1-58 -->
```yaml
name: <workflow_name>
description: <optional string>
schedule: <optional cron string>

vars:                 # optional workflow variables
  <NAME>: <string>

types:                # optional type aliases
  <TypeAlias>: <type_expr>

task_groups:          # optional named task groups
  <group_name>: [<task_name>, ...]

tasks:
  <task_name>:
    executor: container|python|process|cloudrun|library|wasm|<project_executor_alias>
    image: <container_image>  # required for container
    file: <path>         # executor-dependent
    module: <module>     # executor-dependent
    function: <name>     # optional for python
    command: <cmd>       # executor-dependent
    args: [<arg>, ...]   # optional for process/container/wasm
    job: <job_name>      # executor-dependent
    config_file: <path>  # required for custom project executors
    env: { <NAME>: <scalar>, ... }  # optional env vars
    secrets: [{ name: <id>, secret: <ref>, path: <path> }, ...]  # process/python/container only
    depends_on: [<task_name>, ...]
    timeout: <seconds>
    retries: <count>
    retry_delay: <seconds>          # optional fixed retry delay
    on_failure: skip|run            # optional; default skip
    input_type: <object_type_expr>   # optional
    output_type: <type_expr>         # optional
    validate_output: true|false      # optional; default true
    inputs:                          # optional; omit when empty
      <arg_name>: <binding>
    when: <condition string>         # optional
    parallel: <count>                # optional static fan-out only
    concurrency: <count>             # optional with parallel
    parallel_mode: czar|cloud_run_tasks
    wasm_mode: embedded|cli          # wasm only

flows:               # optional curated task subsets
  <flow_name>:
    description: <optional string>
    tasks: [<task_name>, ...] | "*" | {<task_name>: {}}

hooks:               # optional task/run lifecycle hooks
  <hook_name>:
    on: task.start|task.success|task.failed|task.skipped|run.start|run.success|run.failed
    tasks: "*" | [<task_name>, ...]  # task events only
    executor: python|process|cloudrun|library|wasm
    file: <path>
    command: <cmd>
    job: <job_name>
    module: <module>
    function: <name>
    timeout: <seconds>
```

Path rules:
- Relative `file:` paths are resolved from the workflow YAML directory, not from the shell CWD.
- Example: if the workflow file is `workflows/ingest.yaml`, then `file: ../src/tasks/fetch.py` resolves relative to `workflows/`.
- Path-like `command:` values are also resolved from the workflow YAML directory.
- `container` tasks do not resolve `command:` against the host filesystem. Use the executable name inside the image and mount-aware paths such as `/workspace/...` when you need checked-in files.

## Type system
Allowed builtins:
- `str`
- `int`
- `float`
- `bool`
- `date`
- `datetime_notz`
- `datetime_tz`
- `datetime_utc`

Legacy names are rejected:
- `string`, `integer`, `number`, `boolean`, `datetime`, `timestamp`, `timestamp_s`, `timestamp_ms`, `time`, `duration`

Type expression grammar:
- builtin: `<builtin>`
- alias: `types.<Alias>`
- object: `{ field_a: <type_expr>, field_b: <type_expr> }`
- array: `[<type_expr>]` (exactly one element type)
- array shorthand: `list[<type_expr>]`, `List[<type_expr>]`, `array[<type_expr>]`, `Array[<type_expr>]`, or `<type_expr>[]`

Rules:
- Omit `input_type`, `output_type`, and `inputs` when a task has no typed inputs or outputs.
- `input_type` and `output_type` are type-only declarations (no `const`/`ref`).
- `input_type` must resolve to an object type (named task arguments).
- `validate_output: false` disables runtime output-contract enforcement for that task.
- For Python tasks, `validate-workflow` compiles the workflow and checks declared `input_type` / `output_type` against the function annotations when a task file is available.
- Type aliases must not use builtin names.
- Recursive alias cycles are invalid.
- `env` values must be string, number, or boolean literals, or runtime templates such as `{{ discover.output_path }}` / `{{ tasks.discover.output.output_path }}` that resolve to scalar values.
- `secrets` entries are only valid for `process`, `python`, and `container` tasks and each entry must include `name`, `secret`, and `path`.

## Temporal formats
- `date`: `YYYY-MM-DD`
- `datetime_notz`: `YYYY-MM-DDTHH:MM:SS[.fraction]` (timezone/offset not allowed)
- `datetime_tz`: RFC3339 datetime with timezone (`Z` or `+/-HH:MM`)
- `datetime_utc`: RFC3339 datetime with strict `Z` suffix

Compatibility:
- `datetime_utc` is assignable where `datetime_tz` is expected.
- No other implicit temporal coercions.

## Input bindings
A binding must be exactly one of:

<!-- doc-snippet: path=examples/workflows/workflow-schema-reference/input-const.yaml sha256=dbcc970209fb2026b964b954270757084d16f7fade7c9208c7fca302db36f87b lines=1-2 -->
```yaml
<arg>:
  const: <literal>
```

<!-- doc-snippet: path=examples/workflows/workflow-schema-reference/input-ref.yaml sha256=0db0546eea59a93b39492fb068c5ec9e1a63f4867d96a018e20fc1b106e3f380 lines=1-2 -->
```yaml
<arg>:
  ref: <path>
```

No bare scalar/array/object values are allowed in `inputs`.

Reference path grammar:
- `tasks.<task_name>.output[.<field>...]`
- `vars.<NAME>`
- `parallel.index`
- `parallel.count`
- `foreach.item[.<field>...]`
- `foreach.index`
- `foreach.count`

Rules:
- Task output refs are only valid if the current task has `depends_on` that task.
- Self-reference (`tasks.<current>.output`) is invalid.
- Ref field traversal must follow object fields.
- Ref type must be assignable to the target input field type.
- String constants inside bindings support `${{ vars.NAME }}` interpolation at compile time.
- Workflow vars can also be bound directly via `ref: vars.NAME`.
- For deployed workflows, `czar run start ... --input NAME=VALUE` overlays additional runtime values onto `vars.NAME` bindings when the run is materialized.

## Task graph validation
- `depends_on` tasks must exist.
- No self-dependency.
- No duplicate dependencies.
- Task graph must be acyclic.
- `when:` task references must be listed in `depends_on`.

## Executor-specific requirements
- `container`: non-empty `image`
- `python`: non-empty `module` or `file`
- `process`: non-empty `command` or `file`
- `cloudrun`: non-empty `job`
- `library`: non-empty `file`
- `wasm`: non-empty `file`
- custom project executor alias: must exist in nearest `czar.yaml` `executors:` registry and must set `config_file`

## Variables and Interpolation
- Workflow `vars` are string values.
- Override precedence is: workflow `vars`, then `CZAR_VAR_<NAME>`, then `czar run --var NAME=VALUE`.
- Interpolation applies to runtime string fields such as `image`, `file`, `command`, `args`, `job`, `module`, `function`, `when`, hook string fields, and string leaves inside `inputs`.

## Command
Validate a workflow file:

```bash
czar validate-workflow path/to/workflow.yaml
```

On success:
- Prints a success line with workflow name and task count.

On failure:
- Returns non-zero and reports the path/rule that failed.
- File-not-found errors include the declared path, the workflow directory used for resolution, and the final resolved path.

## Valid example
<!-- doc-snippet: path=examples/workflows/workflow-schema-reference/valid-example.yaml sha256=4d6c11efac14ad15e6023ed5ca900f6e47068779ac595dbc9f39e04031b9bbc3 lines=1-31 -->
```yaml
name: fast_demo
vars:
  PROFILE: debug

types:
  TaskResult:
    task: str
    status: str
    timestamp: datetime_utc

tasks:
  task_a:
    executor: python
    module: fast
    function: task_a
    output_type: types.TaskResult

  task_b:
    executor: python
    module: fast
    function: task_b
    depends_on: [task_a]
    input_type:
      a: types.TaskResult
      profile: str
    output_type: types.TaskResult
    inputs:
      a:
        ref: tasks.task_a.output
      profile:
        const: "${{ vars.PROFILE }}"
```

## Parallelism Limits
- `parallel: <count>` is supported today.
- `parallel_mode: cloud_run_tasks` is only valid with `executor: cloudrun`.
- Dynamic `parallel: <upstream field>` and `foreach: <upstream array field>` are supported.
- `foreach:` is only valid with `parallel_mode: czar`; `cloud_run_tasks` cannot pass distinct per-item inputs.
- Runtime fan-out bindings may reference `parallel.index`, `parallel.count`, `foreach.item`, `foreach.index`, and `foreach.count`.
