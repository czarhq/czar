# Known issues

Places where the code on this branch does not do what the rest of these pages, or a user, would expect. Each entry says what happens, where in the code, and which flow it affects. Flow pages link here instead of explaining the bug inline. When a fix lands, delete the entry in the same pull request.

Of the 33 issues below, the fix stack (`stack/00` to `stack/80`, tip `integrate`) fixes 29, partly fixes 2 and leaves 2.

Line numbers are from the commit that added the entry; search for the function name if they have moved.

## `on_failure: run` rarely takes effect

- **What happens:** when a dependency fails for good, the Scheduler marks every waiting dependent `failed` ("upstream failed") without reading `on_failure`. A Task with `on_failure: run` is therefore failed before it is picked. It only runs when its dependency failed by a path that skips this step, such as a failed Deferred job.
- **Where:** `mark_tasks_failed_by_dependency`, called from [status_processing.rs:340](../../../crates/czar-core/src/scheduler/status_processing.rs#L340); the only `on_failure` check is in `execute_task` ([task_execution.rs](../../../crates/czar-core/src/task_execution.rs), `task_runs_despite_failed_dependencies`).
- **Affects:** [failures.md](flows/failures.md#upstream-failure), [schedule.md](flows/schedule.md).
- **Fixed in the fix stack:** `stack/21-on-failure-run` (every backend now leaves a Task with `on_failure: run` pending when its dependency fails).

## Hook results break a batch of status updates

- **What happens:** a Hook runs under a random task id with no Task row. When it ends, its `success` or `failed` StatusUpdate reaches the Scheduler, whose lookup of that id fails with `?`. That aborts the processing of the whole batch, so the other reports drained with it are lost; the error is only logged. Tasks in that batch can stay `running` until their timeout.
- **Where:** [hooks.rs:210](../../../crates/czar-core/src/hooks.rs#L210) (`dispatch_hook`); [status_processing.rs:127](../../../crates/czar-core/src/scheduler/status_processing.rs#L127) and [:238](../../../crates/czar-core/src/scheduler/status_processing.rs#L238).
- **Affects:** [schedule.md](flows/schedule.md#hooks-and-notifications), [failures.md](flows/failures.md).
- **Fixed in the fix stack:** `stack/20-scheduler-fault-isolation` (the Scheduler drops Hook results before the lookup, and skips any other unknown task id instead of failing the batch).

## A fan-out error blocks dispatch

- **What happens:** a `parallel: <ref>` or `foreach: <ref>` placeholder whose output is missing, not a count, or an empty list makes the whole dispatch step return an error. Nothing in that batch is dispatched, the placeholder stays `pending`, and the same error repeats every tick while it is among the 100 oldest ready Tasks.
- **Where:** the `?` after `materialize_dynamic_fanout` in [processing.rs:399](../../../crates/czar-core/src/scheduler/processing.rs#L399); [dynamic_fanout.rs](../../../crates/czar-core/src/scheduler/dynamic_fanout.rs).
- **Affects:** [schedule.md](flows/schedule.md#when-a-task-is-ready).
- **Fixed in the fix stack:** `stack/20-scheduler-fault-isolation` (a placeholder that cannot fan out is failed with the error, and the rest of the batch goes on).

## `when` cannot read a fanned-out Task

- **What happens:** a fanned-out dependency's outputs are loaded under `x[0]`, `x[1]`, …, but `when` looks up `tasks.x.output` by the plain name, finds nothing, and fails the Task. (`ref` bindings handle this case and return a list.)
- **Where:** dependency outputs are loaded by the expanded names in [processing.rs:328](../../../crates/czar-core/src/scheduler/processing.rs#L328); the lookup is in [condition.rs:160](../../../crates/czar-core/src/condition.rs#L160).
- **Affects:** [schedule.md](flows/schedule.md#inputs-references-and-conditions).
- **Fixed in the fix stack:** `stack/11-when-conditions` (`when` resolves task refs like `ref` bindings, so a fanned-out Task gives a list; `stack/60-when-any-all` adds `any()` and `all()` to test that list).

## Flow Snapshots drop Hooks, signatures and some overrides

- **What happens:** every `POST /api/runs`, and the CLI with `--flow` or selectors, builds a new Snapshot from the flow's tasks. That Snapshot has no Hook rows (Hooks never fire for Runs started from the Web UI or TUI), no output signatures (`output_type` is not checked), and rejects a task with a static `parallel: N` because it looks Workflow tasks up by their YAML name. The flow's `env`, `retry_delay` and `on_failure` overrides are ignored, and an `inputs` override is stored without var interpolation.
- **Where:** `build_snapshot_tasks_for_flow` in [workflow_groups.rs:59](../../../crates/czar-web/src/workflow_groups.rs#L59), and its copy in [trigger.rs](../../../crates/czar-cli/src/commands/trigger.rs).
- **Affects:** [start.md](flows/start.md#which-snapshot-a-run-pins), [schedule.md](flows/schedule.md#hooks-and-notifications).
- **Fixed in the fix stack:** `stack/33-flow-start-fanout` (the API and the CLI share `build_flow_snapshot_tasks`, which copies the stored rows with their instances, signatures and Hooks, and applies the flow's overrides on top with vars interpolated).

## The schedule API accepts any cron expression

- **What happens:** `PATCH /api/workflows/{name}/schedule` stores the expression without parsing it and keeps the old next run time. An invalid expression is then logged as invalid at every check, and no Run starts. (Schedules from workflow YAML are parsed with `croner`: five fields with an optional leading seconds field.)
- **Where:** [api_routes_control.rs](../../../crates/czar-web/src/api_routes/api_routes_control.rs).
- **Affects:** [start.md](flows/start.md#schedules).
- **Fixed in the fix stack:** `stack/03-web-handlers` (the PATCH endpoint parses the expression like the Scheduler, rejects an invalid one with 400, and recomputes the next run time).

## Replacing a Workflow does not update its schedule

- **What happens:** the schedule from the YAML is only stored when the Workflow is created. Re-applying it through `czar run FILE`, the API or a reload keeps the old cron expression; only `czar workflow deploy` writes it again. A Workflow created by the API or a reload also starts with its schedule disabled.
- **Where:** `persist_schedule_on_create` in [workflow_yaml.rs:172](../../../crates/czar-core/src/services/workflow_yaml.rs#L172).
- **Affects:** [define.md](flows/define.md#which-entry-point-does-what), [start.md](flows/start.md#schedules).
- **Not fixed yet.**

## Worker tasks fail after 30 seconds

- **What happens:** the Worker waits at most 30 seconds for a Task's result, then answers `failed` with "Execution timed out waiting for completion". The Task process keeps running on the Worker.
- **Where:** `TERMINAL_UPDATE_TIMEOUT` in [dispatch.rs:9](../../../crates/czar-executors/src/dispatch.rs#L9).
- **Affects:** [execute-worker.md](flows/execute-worker.md).
- **Fixed in the fix stack:** `stack/24-executor-contract` (the Worker waits up to the Task's own `timeout_seconds` and then stops the Task; `stack/35-worker-sync-and-timeouts` also drops the Scheduler's fixed 300-second request timeout).

## Worker tasks always run as process

- **What happens:** the Worker executor sends `executor_type: process` for every Task. Python and container Tasks still work through their params; library, Wasm and Cloud Run Tasks run as shell commands and fail.
- **Where:** [worker/executor.rs:58](../../../crates/czar-executors/src/worker/executor.rs#L58).
- **Affects:** [execute-worker.md](flows/execute-worker.md).
- **Fixed in the fix stack:** `stack/08-worker-executor-type` (the Worker executor sends the Task's own executor type).

## A Worker call blocks the Scheduler

- **What happens:** the Worker executor's `execute` waits for the Worker's answer, and the Scheduler's dispatch step waits for every `execute` in the batch. While a Worker Task runs (up to 30 seconds, or 300 if the Worker hangs), the Scheduler records nothing and dispatches nothing else. Library and embedded Wasm Tasks block the same way for as long as they run.
- **Where:** `buffer_unordered(...).collect()` in [processing.rs:446](../../../crates/czar-core/src/scheduler/processing.rs#L446); [worker/executor.rs](../../../crates/czar-executors/src/worker/executor.rs).
- **Affects:** [execute-worker.md](flows/execute-worker.md), [schedule.md](flows/schedule.md).
- **Fixed in the fix stack:** `stack/24-executor-contract` (the Worker, library and embedded Wasm executors start the Task in the background, and `execute` returns at once).

## Some failures are counted twice

- **What happens:** the Worker, library and embedded Wasm executors report a failure both as a `failed` StatusUpdate and as an error from `execute`. The Scheduler handles both, so with retries one failure can use two attempts.
- **Where:** [worker/executor.rs:104](../../../crates/czar-executors/src/worker/executor.rs#L104), [library/executor.rs:196](../../../crates/czar-executors/src/library/executor.rs#L196), [wasm/executor.rs](../../../crates/czar-executors/src/wasm/executor.rs).
- **Affects:** [execute.md](flows/execute.md), [failures.md](flows/failures.md#retries).
- **Fixed in the fix stack:** `stack/24-executor-contract` (these executors report the result only as a StatusUpdate; `execute` returns an error only when nothing started).

## SDK error results count as success

- **What happens:** the Rust SDK's library and Wasm macros return `{"error": "..."}` on bad input or output. Both executors accept any JSON as the output, so the Task succeeds with that error object as its output.
- **Where:** [library/executor.rs](../../../crates/czar-executors/src/library/executor.rs) (`execute_library`), [wasm/embedded.rs](../../../crates/czar-executors/src/wasm/embedded.rs); the SDK side in [czar lib.rs](../../../crates/czar/src/lib.rs) (`__czar_library_run_impl`) and [wasm.rs](../../../crates/czar/src/wasm.rs).
- **Affects:** [execute-local.md](flows/execute-local.md), [task-sdks.md](task-sdks.md#rust).
- **Fixed in the fix stack:** `stack/09-rust-sdk` (both executors fail the Task on an unprefixed `{"error": "..."}`; SDK outputs carry the `CZAR_OUTPUT:` prefix, so a user output with an `error` key still succeeds).

## A library Task with non-JSON output never finishes

- **What happens:** when a library returns a string that is not JSON, the Executor sends neither `success` nor `failed`. The Task stays `running` until its timeout.
- **Where:** the `if let Ok(output_value)` without an `else` in [library/executor.rs:144](../../../crates/czar-executors/src/library/executor.rs#L144).
- **Affects:** [execute-local.md](flows/execute-local.md#library).
- **Fixed in the fix stack:** `stack/09-rust-sdk` (output that is not JSON now fails the Task).

## Invalid UTF-8 output stops log reading

- **What happens:** a stdout or stderr line that is not valid UTF-8 makes the reader stop for that stream. A Task that keeps writing can then block on the full pipe until its timeout.
- **Where:** `read_line(...).await.unwrap_or_default()` in [process/logs.rs:70](../../../crates/czar-executors/src/process/logs.rs#L70).
- **Affects:** [execute-local.md](flows/execute-local.md#reading-the-output).
- **Fixed in the fix stack:** `stack/07-process-output` (lines are read as bytes and decoded lossily, so reading goes on).

## Cloud Run status is read from the condition type

- **What happens:** the Cloud Run executor reads the first status condition's `type` (for example `Completed`), not its state, so a running execution can be reported as success. The czar-gcp tracker reads the state correctly.
- **Where:** [cloud_run/client.rs:260](../../../crates/czar-executors/src/cloud_run/client.rs#L260); compare `map_cloudrun_execution` in [czar-gcp cloudrun.rs](../../../crates/czar-gcp/src/job_tracker/cloudrun.rs).
- **Affects:** [execute-cloud-run.md](flows/execute-cloud-run.md).
- **Fixed in the fix stack:** `stack/06-cloud-run-executor` (it reads the `Completed` condition's state through the v2 API; `stack/49-cloud-run-cancel` also ends Tasks whose execution was cancelled or deleted).

## Cloud Run executions are lost on restart

- **What happens:** started executions are remembered only in the Executor's memory. After a restart nothing polls them; their Tasks wait for the timeout, and a retry starts a second execution.
- **Where:** `active_executions` in [cloud_run/client.rs:58](../../../crates/czar-executors/src/cloud_run/client.rs#L58).
- **Affects:** [execute-cloud-run.md](flows/execute-cloud-run.md), [failures.md](flows/failures.md#restart-and-shutdown).
- **Fixed in the fix stack:** partly, `stack/50-kill-after-restart`: a restarted Scheduler can now cancel the old execution when its Task times out, so a retry no longer runs next to it, but nothing polls the execution after a restart, so the Task still waits for its timeout.

## Cloud Run Tasks from `czar run` and reload use project `local`

- **What happens:** `czar run FILE` and the definitions reload store the Workflow's project and region as `local`, which the Cloud Run executor uses to build `https://local-run.googleapis.com/...`, so those Tasks fail.
- **Where:** [execute.rs:162](../../../crates/czar-cli/src/commands/execute.rs#L162), [reload_workflow_definitions.rs:293](../../../crates/czar-web/src/handlers/reload_workflow_definitions.rs#L293).
- **Affects:** [execute-cloud-run.md](flows/execute-cloud-run.md), [define.md](flows/define.md).
- **Not fixed yet.**

## Rust SDK deferrables are rejected

- **What happens:** the Rust SDK's `Deferrable::to_json` serializes only the struct's fields, without `service_type` (and, for some types, without `job_id`). The Scheduler logs "Invalid deferrable data" and creates no Deferred job, and the Task waits until its timeout.
- **Where:** [czar deferrables.rs](../../../crates/czar/src/deferrables.rs) (`to_json` of each type); the check in [status_processing.rs](../../../crates/czar-core/src/scheduler/status_processing.rs) (`process_status_updates`).
- **Affects:** [deferred.md](flows/deferred.md), [task-sdks.md](task-sdks.md#deferrables).
- **Fixed in the fix stack:** `stack/09-rust-sdk` (`to_json` now adds `service_type`, and `job_id` when the type has none of its own).

## Python tasks cannot return a Deferrable object

- **What happens:** the Python runner's JSON encoder does not know the SDK's `Deferrable` classes, so returning one raises `TypeError` and the Task fails. Task code has to return `{"deferred": [job.to_dict()]}`.
- **Where:** `_json_default` in [run_task.py:14](../../../crates/czar-executors/runtime/python/run_task.py#L14).
- **Affects:** [deferred.md](flows/deferred.md), [task-sdks.md](task-sdks.md#deferrables).
- **Fixed in the fix stack:** `stack/10-python-runtime` (the runner turns a returned `Deferrable`, or a list of them, into `{"deferred": [...]}`).

## Deferred Tasks keep no output

- **What happens:** the output that carried the `deferred` list is not stored, and finishing the Task stores none, so downstream Tasks cannot read a value from a deferred Task.
- **Where:** the `continue` in the `deferred` branch of [status_processing.rs:96](../../../crates/czar-core/src/scheduler/status_processing.rs#L96); `process_job_completions` in [status_processing_completion.rs:252](../../../crates/czar-core/src/scheduler/status_processing_completion.rs#L252).
- **Affects:** [deferred.md](flows/deferred.md).
- **Fixed in the fix stack:** `stack/25-deferred-jobs` (the output is stored when the jobs are recorded, and kept when the Task finishes).

## Deferred jobs with an unknown service type poll forever

- **What happens:** a Deferred job whose `service_type` has no tracker (for example `bigquery` in a build without `gcp`) fails inside the poll task, and the error is dropped without a log. The job stays `pending` forever and the Task waits until its timeout.
- **Where:** [triggerer.rs:197](../../../crates/czar-core/src/triggerer.rs#L197); the result check at [:178](../../../crates/czar-core/src/triggerer.rs#L178) only sees task panics.
- **Affects:** [deferred.md](flows/deferred.md#polling).
- **Fixed in the fix stack:** `stack/26-triggerer` (a job with no tracker is failed with "No job tracker registered for service type"; `stack/34-deferred-job-guards` logs every poll error).

## Cancel and timeouts do not stop work

- **What happens:** cancelling a Run and timing out a Task only change rows. No Executor can stop a process, a Worker call, a Cloud Run execution or a Deferred job, so the work runs to its end.
- **Where:** the `Executor` trait has no stop method ([executor.rs](../../../crates/czar-core/src/executor.rs)); `cancel_run` in each backend; `enforce_timeouts` in [status_processing.rs](../../../crates/czar-core/src/scheduler/status_processing.rs).
- **Affects:** [control.md](flows/control.md), [failures.md](flows/failures.md#timeouts).
- **Fixed in the fix stack:** partly, `stack/23-attempt-scoped-updates-and-kill`, `stack/47-cancel-container-wasm`, `stack/48-worker-cancel`, `stack/49-cloud-run-cancel` and `stack/69-worker-cloud-run-jobs` stop processes, containers, embedded Wasm, Worker calls, Cloud Run executions and Cloud Run deferred jobs: a library Task still runs to its end, and BigQuery, Dataproc and custom HTTP deferred jobs are only marked cancelled.

## Late results overwrite cancelled Tasks

- **What happens:** work that was still out reports later, and the Scheduler writes that status over `cancelled`: Postgres' guard only protects `success` and `failed` rows, and SQLite and D1 have none. A finished Deferred job does the same. If every Task then counts as ended, the cancelled Run is moved to `success` or `failed`. The same happens to a Task that timed out and was retried.
- **Where:** the `WHERE` clause of `batch_update_task_status_impl` in [czar-store-postgres tasks.rs:138](../../../crates/czar-store-postgres/src/tasks.rs#L138); [czar-store-sqlite tasks.rs](../../../crates/czar-store-sqlite/src/tasks.rs); `process_job_completions`.
- **Affects:** [control.md](flows/control.md), [failures.md](flows/failures.md).
- **Fixed in the fix stack:** `stack/22-status-transition-guards` (every backend checks allowed transitions, so a `cancelled` Task or Run is never moved; `stack/23-attempt-scoped-updates-and-kill` drops late results from an earlier attempt).

## Worker sync needs Python task files on the orchestrator

- **What happens:** compiling a synced workflow still resolves and inspects `python` task files, relative to the orchestrator's current directory, even with relaxed validation. Syncing a Worker whose workflow has `python` tasks with `file:` fails unless the same files exist on the orchestrator; and the paths resolved there are what the Worker later receives.
- **Where:** `resolve_file` and `introspect_python_signature` in [compiled.rs:207](../../../crates/czar-core/src/compiled.rs#L207); [worker_sync.rs](../../../crates/czar-core/src/services/worker_sync.rs) (compiles with root `.`).
- **Affects:** [define.md](flows/define.md), [execute-worker.md](flows/execute-worker.md).
- **Fixed in the fix stack:** `stack/32-worker-sync-remote-files` (worker sync compiles with `remote_files`, which keeps the declared paths and skips resolving and inspecting files on the orchestrator).

## `czar run FILE` has no Worker tokens

- **What happens:** the Scheduler started by `czar run FILE` has no Worker registrations, but serves every pending Run in the Store. Tasks of a Worker Workflow it picks up are sent without a token.
- **Where:** `ExecutorManager::new()` in [execute.rs:210](../../../crates/czar-cli/src/commands/execute.rs#L210).
- **Affects:** [execute-worker.md](flows/execute-worker.md).
- **Fixed in the fix stack:** `stack/35-worker-sync-and-timeouts` (it loads the Worker registrations from `--config` and `CZAR_WORKERS_FILE`, as `czar serve` does).

## Registering a Worker over the API sends no token

- **What happens:** `POST /api/workers` has no token field, so it cannot sync a Worker that requires one. Tasks still get a token if the same URL is in the configured Worker registrations.
- **Where:** `register_worker` in [workers.rs:32](../../../crates/czar-web/src/handlers/workers.rs#L32).
- **Affects:** [define.md](flows/define.md), [access.md](flows/access.md).
- **Fixed in the fix stack:** `stack/43-worker-token-registration` (the request takes a `token`, kept in memory for that Worker's Tasks).

## The standalone API has no allowed-origin option

- **What happens:** `czar-web` has no `--allowed-origin`, so a browser UI on another origin cannot call it; only `czar serve` has the flag.
- **Where:** `Args` in [czar-web main/bootstrap.rs:77](../../../crates/czar-web/src/main/bootstrap.rs#L77).
- **Affects:** [access.md](flows/access.md#the-request-guard).
- **Fixed in the fix stack:** `stack/42-api-auth-modes` (`czar-web` now takes `--allowed-origin`).

## The Cloudflare container has no auth mode

- **What happens:** the container starts `czar serve --addr 0.0.0.0:8080`. On a non-loopback address the API refuses to start without `--auth` or `CZAR_AUTH`/`CZAR_AUTH_TOKEN`, and the container environment sets neither.
- **Where:** [Dockerfile:47](../../../deploy/cloudflare/full/Dockerfile#L47), `envVars` in [src/index.ts:7](../../../deploy/cloudflare/full/src/index.ts#L7).
- **Affects:** [access.md](flows/access.md), [building-blocks.md](building-blocks.md#deployments).
- **Fixed in the fix stack:** `stack/42-api-auth-modes` (the container starts with `--auth external`, behind the Cloudflare Worker's Basic auth).

## D1 inserts all Tasks of a Run in one statement

- **What happens:** the D1 backend inserts every Task of a Run in one statement with 11 bound parameters per Task. Cloudflare caps a D1 statement at 100 bound parameters, so Runs with more than 9 Tasks can fail to start. (Inferred from the limit; not reproduced.)
- **Where:** `batch_create_dag_tasks_impl` in [czar-store-d1 tasks_core.rs:55](../../../crates/czar-store-d1/src/database/tasks_core.rs#L55).
- **Affects:** [schedule.md](flows/schedule.md#building-a-runs-tasks), [storage.md](storage.md).
- **Fixed in the fix stack:** `stack/16-d1-fixes` (inserts are split into statements of at most 100 parameters; `stack/55-d1-atomic-batches` makes them all-or-nothing).

## Reload walks every directory

- **What happens:** the definitions reload skips only `docs/`, `profiles/` and `workflow-schema-reference/`. It scans `.git`, `node_modules` and `target`, and follows symlinked directories, which is slow and can loop.
- **Where:** `should_ignore_workflow_support_dir` in [reload_workflow_definitions.rs:110](../../../crates/czar-web/src/handlers/reload_workflow_definitions.rs#L110).
- **Affects:** [define.md](flows/define.md#loading-the-yaml).
- **Fixed in the fix stack:** `stack/03-web-handlers` (reload walks with `walkdir`, which does not follow symlinks, and skips hidden directories, `target` and `node_modules`).

## Related

- [writing.md](writing.md): how pages link here.
- [domain-model.md](domain-model.md#lifecycles): the lifecycles these issues break.
