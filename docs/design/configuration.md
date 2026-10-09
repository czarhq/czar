# Configuration

_Updated at: 2026-05-22_

<!-- Status: implemented for workflow task settings, workflow vars, and scheduler tuning. -->

## Examples Covered

1. [Quickstart guide example](../../examples/workflows/quickstart-guide/)
2. [Tutorial ELT pipeline](../../examples/workflows/tutorial-elt-pipeline/)

## Scope

This page documents the configuration surface that exists today.

This page covers:

- task retries
- per-task retry delay override
- task timeout
- task `env:` blocks with scalar values
- task `secrets:` pre-mounted file bindings for process, python, and container tasks
- workflow variables and interpolation
- local `czar.yaml` `env_file:` loading for CLI defaults
- local `czar.yaml` `executors:` registration for project custom Python executors
- local `czar.yaml` `notifications:` for scheduler-managed failure delivery
- conditional execution
- parallelism settings
- scheduler tuning config for `czar serve` / `czar run`

## Task Retries

Retries are configured per task:

<!-- doc-snippet: path=examples/reference/future-configuration/12-current-retries.yaml sha256=212a84fcac30d036728e33cd36deddd2d7ebab63c480e85f58631216514224f1 lines=1-5 -->
```yaml
tasks:
  flaky_task:
    executor: python
    file: tasks_flaky.py
    retries: 3
```

Current behavior:

- `retries` is the maximum number of retry attempts after the initial failure path begins.
- Backoff is automatic and exponential by default: `1s`, `2s`, `4s`, `8s`, ... capped at `60s`.
- `retry_delay` overrides that backoff with a fixed per-attempt delay in seconds.

Example:

<!-- doc-snippet: path=examples/reference/future-configuration/01-retries.yaml sha256=5474b42553ac9d50102be51226aedbef1fcdfa73525d925a733f602177642f10 lines=1-6 -->
```yaml
tasks:
  flaky_task:
    executor: python
    file: tasks/flaky.py
    retries: 3
    retry_delay: 60  # seconds
```

## Task Timeouts

Timeouts are configured per task in seconds:

<!-- doc-snippet: path=examples/reference/future-configuration/13-current-timeout.yaml sha256=cfd4ff4769e74a1b88386d23e386796eac7846835cd372ed5de8485728cd579b lines=1-5 -->
```yaml
tasks:
  slow_task:
    executor: process
    command: ./slow-job
    timeout: 1800
```

Current behavior:

- `timeout` defaults to `300` seconds for workflow definitions.
- Running tasks that exceed their timeout are marked failed with a timeout error.

## Task Environment Variables

Static environment variables are configured per task:

<!-- doc-snippet: path=examples/reference/future-configuration/04-static-env.yaml sha256=d2fb219884b7fd49618386466eec0cb7b878d9290595dabe1efd96a9f5c947c0 lines=1-8 -->
```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py
    env:
      LOG_LEVEL: debug
      API_URL: https://api.example.com
      MAX_RETRIES: "3"
```

Current behavior:

- `env:` must be an object
- each env value may be a scalar literal or a runtime template like `{{ discover.output_path }}`
- string values support `${{ vars.NAME }}` interpolation at compile time
- values are passed through to process, container, Python, and Cloud Run task execution
- cloud/provider secrets should be injected by infra as ordinary environment variables before Czar runs

Upstream-output env example:

<!-- doc-snippet: path=examples/reference/future-configuration/06-env-references.yaml sha256=c5e22a6486afca515873611650bb30621c301f98a1036b3fa07fa812dfb1e04b lines=1-11 -->
```yaml
tasks:
  discover:
    executor: python
    file: tasks/discover.py

  process:
    executor: python
    file: tasks/process.py
    depends_on: [discover]
    env:
      DATA_PATH: "{{ discover.output_path }}"
```

For env templates:

- `{{ discover.output_path }}` is shorthand for the upstream task output field
- explicit refs like `{{ tasks.discover.output.output_path }}` also work
- the referenced task must still be listed in `depends_on`
- the resolved env value must be a string, number, or boolean

Secret-backed env example:

<!-- doc-snippet: path=examples/reference/future-configuration/20-current-secret-env.yaml sha256=d219a7798e3248c5f69bedaa08d93bb2fd534564df1ec4ec8d3902110060772f lines=1-9 -->
```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py
    env:
      API_KEY:
        secret: projects/my-project/secrets/api-key/versions/latest
      DB_PASSWORD:
        secret: projects/my-project/secrets/db-password/versions/latest
```

## Mounted Secret Files

Mounted secret files are configured per task with `secrets:`:

<!-- doc-snippet: path=examples/reference/future-configuration/08-file-secrets.yaml sha256=759ff12ea0753809d90a9fd1daa9957b0ec9bac1b8625722132732810618c3ba lines=1-8 -->
```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py
    secrets:
      - name: service-account
        secret: projects/my-project/secrets/sa-key/versions/latest
        path: .czar/secrets/sa-key.json
```

Current behavior:

- `secrets:` is supported for `process`, `python`, and `container` tasks
- each entry must include `name`, `secret`, and `path`
- `secret` is an opaque label for the infra-mounted file, not a provider lookup reference
- relative `path` values are resolved against the executor working directory
- Czar expects the file at `path` to already exist and does not fetch or write secret contents
- Czar exports a convenience env var named `CZAR_SECRET_<NAME>_PATH`, with non-alphanumeric characters in `name` normalized to `_`
- infra-mounted files are left in place after the task exits

Example task code:

<!-- doc-snippet: path=examples/reference/future-configuration/09-file-secrets.py sha256=2e92bb246b337ed1abe59414509520d54a09370374578455587afcd4126e8268 lines=1-8 -->
```python
import json
import os

@task
def main(input: Input) -> Output:
    with open(os.environ["CZAR_SECRET_SERVICE_ACCOUNT_PATH"]) as f:
        credentials = json.load(f)
    # ...
```

## Variables And Interpolation

Workflow variables are string values:

<!-- doc-snippet: path=examples/reference/future-configuration/14-current-vars.yaml sha256=a39f9cd3fbd56319a528f6807738410159817ed3260342144a553e3a1bdfd647 lines=1-3 -->
```yaml
vars:
  BUILD_PROFILE: debug
  JOB_NAME: nightly-users
```

They can be used in runtime string fields and string constants inside bindings:

<!-- doc-snippet: path=examples/reference/future-configuration/15-current-interpolation.yaml sha256=f52a79880a76d7b6d211dd3544f057cddddfa6d89b6d9efba4263294e79a12f5 lines=1-4 -->
```yaml
tasks:
  build:
    executor: process
    command: target/${{ vars.BUILD_PROFILE }}/my-job
```

Override precedence:

1. workflow `vars`
2. `CZAR_VAR_<NAME>`
3. `czar run --var NAME=VALUE`

## Local env_file

Local CLI commands can load a project env file from `czar.yaml`:

<!-- doc-snippet: path=examples/reference/future-configuration/10-env-file.yaml sha256=15bd8f165f7d1a72aca8b667a6d617d50fcce71a8f7ed55d4e1ba709c36be61e lines=1-2 -->
```yaml
# czar.yaml (dev config)
env_file: .env
```

Current behavior:

- the CLI searches the current directory and its ancestors for `czar.yaml`
- if `env_file:` is present, the file path is resolved relative to that `czar.yaml`
- entries are loaded into the process environment before backend/database resolution
- already-set environment variables still win; `env_file` only provides defaults

## Project Executor Registry

Projects can register custom Python executor aliases in `czar.yaml`:

<!-- doc-snippet: path=examples/reference/future-executors/03-register-executor.yaml sha256=3d9608297cf54e9651d18c85e7f7fd3897fd712cab23287431587d169382b070 lines=1-8 -->
```yaml
# czar.yaml
executors:
  bigquery:
    type: executors.bigquery:BigQueryExecutor
    config:
      project: my-gcp-project
      location: US
      cost_per_tb: 5.0
```

Current behavior:

- the workflow loader searches the workflow directory and its ancestors for `czar.yaml`
- aliases must not shadow built-in executor names such as `python` or `process`
- `type:` must be a Python import target in `module:ClassName` form
- `config:` is passed to the executor class constructor
- tasks using a project executor alias must provide `config_file:`
- project config loading here is for local workflow compilation and execution, not remote API client auth

## Project Notifications

Local scheduler commands can also load notification defaults from `czar.yaml`:

<!-- doc-snippet: path=examples/reference/future-failure-handling/06-built-in-notifications.yaml sha256=d9834a30246bac7d4664ee75a4195e2d71970a71aab0dc582cf4e4cb834fd9ed lines=1-7 -->
```yaml
# czar.yaml
notifications:
  on_failure:
    email:
      to: [team@example.com]
    slack:
      webhook_url: ${SLACK_WEBHOOK_URL}
```

Current behavior:

- the CLI searches the current directory and its ancestors for `czar.yaml`
- `czar serve` and local `czar run` / `czar execute` scheduler startup load `notifications:` from that file
- an explicit scheduler config file still wins if it already defines `notifications`

## Input Bindings

Task inputs are configured explicitly through `inputs`:

<!-- doc-snippet: path=examples/reference/future-configuration/16-current-input-bindings.yaml sha256=894145d22ae66af022aeb2526236047c56573d82e80fa43d7563b5504b25f370 lines=1-10 -->
```yaml
tasks:
  summarize:
    executor: python
    file: tasks_summarize.py
    input_type:
      fetch:
        data: str
    inputs:
      fetch:
        ref: tasks.fetch.output
```

Supported binding types:

- `const:`
- `ref:`

See [workflow-schema.md](workflow-schema.md) for the exact binding grammar.

## Conditional Execution

Tasks can be guarded with `when:`:

<!-- doc-snippet: path=examples/reference/future-configuration/17-current-when.yaml sha256=be29137edf762d4575c44353987497f3487a63a68ff72d7368853d759c9aca55 lines=1-6 -->
```yaml
tasks:
  publish:
    executor: process
    command: ./publish.sh
    depends_on: [check]
    when: tasks.check.output.ready == true
```

Current behavior:

- `when:` expressions reference upstream outputs.
- Referenced tasks must also be listed in `depends_on`.
- A false condition marks the task `skipped`.

## Parallelism

Parallel settings are configured per task:

<!-- doc-snippet: path=examples/reference/future-configuration/18-current-parallel.yaml sha256=1ceaf1d0505baf4a89ec9dd32c42faf63893a3622f3bd085c630106a41303e43 lines=1-6 -->
```yaml
tasks:
  process_item:
    executor: python
    file: tasks_process.py
    foreach: discover.items
    concurrency: 8
```

Implemented settings:

- `parallel: <count>`
- `parallel: <upstream field>`
- `foreach: <upstream array field>`
- `concurrency: <count>`
- `parallel_mode: czar|cloud_run_tasks`

See [parallelization.md](parallelization.md) for the execution model.

## Scheduler Tuning Config

`czar serve` and `czar run` accept a scheduler config YAML via `--config`.

Current fields:

<!-- doc-snippet: path=examples/reference/future-configuration/19-current-scheduler-config.yaml sha256=99cceb239860b7cf6d2065aea7383f68fb667daf3cd97960be74bc2fc2ab6bd4 lines=1-6 -->
```yaml
poll_interval_secs: 0.5
max_tasks_per_batch: 100
max_concurrent_dispatches: 10
max_concurrent_status_checks: 50
db_pool_size: 10
enable_triggerer: true
```

These can also be supplied through environment variables:

- `POLL_INTERVAL_SECS`
- `MAX_TASKS_PER_BATCH`
- `MAX_CONCURRENT_DISPATCHES`
- `MAX_CONCURRENT_STATUS_CHECKS`
- `DB_POOL_SIZE`
- `ENABLE_TRIGGERER`

This config tunes the scheduler loop. It is not a workflow-definition file.
