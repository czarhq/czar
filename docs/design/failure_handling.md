# Failure Handling

_Updated at: 2026-05-22_

<!-- Status: implemented for retries, timeout enforcement, cancellation, hooks, built-in notifications, and manual retry CLI. -->

## Examples Covered

1. [Quickstart guide example](../../examples/workflows/quickstart-guide/)
2. [Tutorial ELT pipeline](../../examples/workflows/tutorial-elt-pipeline/)

## Default Behavior

When a task fails after retry handling is exhausted:

1. The task is marked `failed`
2. Downstream tasks that depend on it are marked failed with `upstream failed`
3. The run completes as `failed` once all tasks are terminal
4. `task.failed` and `run.failed` hooks may fire if configured

`skipped` is a different path:

- a task is `skipped` when its `when:` condition evaluates false
- tasks that depend on a skipped task are also skipped

## Retries

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

- retries are automatic after executor-reported failure
- backoff is exponential: `1s`, `2s`, `4s`, `8s`, ... capped at `60s`
- retry attempts are tracked on the task row
- a retry appends a scheduler log line noting the next attempt
- `retry_delay` switches that task to a fixed per-attempt delay in seconds

## Manual Retry

The CLI can reset failed sections of a run back to `pending`:

```bash
czar run retry <run_id>
czar run retry <run_id> --task transform
```

Current behavior:

- the selected failed or cancelled root task is reset
- failed, skipped, and cancelled downstream tasks are also reset
- successful tasks are left alone
- the run status is set back to `pending`

## Cancellation

Cancel a run with:

```bash
czar run cancel <run_id>
```

Current behavior:

- the run is marked `cancelled`
- pending, dispatched, and running tasks for that run are transitioned to cancelled by the repository layer
- no new tasks are dispatched for that run afterward

## Timeouts

Task timeouts are enforced by the scheduler:

<!-- doc-snippet: path=examples/reference/future-configuration/13-current-timeout.yaml sha256=cfd4ff4769e74a1b88386d23e386796eac7846835cd372ed5de8485728cd579b lines=1-5 -->
```yaml
tasks:
  slow_task:
    executor: process
    command: ./slow-job
    timeout: 1800
```

When the timeout is exceeded, the scheduler emits a failed status update with an error like `timeout after <N>s`.

## Conditions And Skip Propagation

Conditional tasks use `when:`:

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

- if the condition evaluates false, the task is marked `skipped`
- tasks depending on a skipped task are also skipped
- skipped tasks can trigger `task.skipped` hooks

## Run Despite Failure

Tasks can opt into failure-aware downstream execution with `on_failure: run`:

<!-- doc-snippet: path=examples/reference/future-failure-handling/02-run-despite-failure.yaml sha256=4688c22e9f97463f18e2d4af49699dd868eb9a2e3f0941bb3f50aa35e136b167 lines=1-28 -->
```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py

  transform:
    executor: python
    file: tasks/transform.py
    depends_on: [extract]

  load:
    executor: python
    file: tasks/load.py
    depends_on: [transform]

  # Notification runs regardless of success/failure
  notify:
    executor: python
    file: tasks/notify.py
    depends_on: [load]
    on_failure: run  # Run even if load fails

  # Cleanup always runs
  cleanup:
    executor: python
    file: tasks/cleanup.py
    depends_on: [extract, transform, load]
    on_failure: run
```

Current behavior:

- default policy is still to stop on failed or cancelled upstream dependencies
- `on_failure: run` allows the task to dispatch even when one or more dependencies failed or were cancelled
- `skipped` upstream dependencies still propagate as `skipped`
- unresolved `ref: tasks.<dep>.output...` bindings from failed or cancelled upstream tasks resolve to `null` for that task

## Built-In Failure Notifications

The scheduler can send failure notifications from `czar.yaml`:

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

- notifications are configured at the scheduler/project level, not per workflow
- delivery happens when a run reaches terminal `failed`
- Slack uses an incoming webhook URL provided directly to the scheduler config
- email delivery uses the local `sendmail` binary and a recipient list from `notifications.on_failure.email.to`
- hooks and built-in notifications can be used together

## Hooks For Cleanup And Notifications

The supported way to respond to failures is through hooks:

<!-- doc-snippet: path=examples/reference/future-failure-handling/09-current-hook.yaml sha256=2f6296d96ac3cb02f0e51d77ac03ffbb7fa7b7c45edc3033b9c8d4f19f0035d3 lines=1-5 -->
```yaml
hooks:
  notify_failure:
    on: run.failed
    executor: python
    file: hooks_notify.py
```

Supported hook events:

- `task.start`
- `task.success`
- `task.failed`
- `task.skipped`
- `run.start`
- `run.success`
- `run.failed`

Hooks are the implemented workflow-level mechanism for cleanup and side effects. Use built-in notifications for scheduler-managed Slack or email delivery on failed runs.

## Error Visibility

Current places where task failure information appears:

- task error field in state
- task logs
- `czar logs <run_id>`
- `czar tasks <run_id>`
- `czar status <run_id>`
- workflow task state snapshots used by the UI/API
