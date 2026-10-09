# Operations

_Updated at: 2026-05-22_

<!-- Status: implemented in the CLI and current database-backed runtime. -->

## Examples Covered

1. [Quickstart guide example](../../examples/workflows/quickstart-guide/)
2. [README quickstart example](../../examples/workflows/readme-quickstart/)

## Local Development

Start the scheduler and UI:

```bash
czar serve
```

Shorthand:

```bash
czar dev
```

Run a workflow YAML locally from disk:

```bash
czar run workflow.yaml
```

Optional local-run helpers:

```bash
czar run workflow.yaml --flow ingest_only
czar run workflow.yaml --select task_a --select task_b+
czar run workflow.yaml --var PROFILE=debug
czar run workflow.yaml --silent
czar run workflow.yaml --verbose
```

## Deployed Workflows

Persist a workflow definition into the database:

```bash
czar workflow deploy workflow.yaml
```

Inspect or remove deployed workflows:

```bash
czar workflow list
czar workflow status my_workflow
czar workflow delete my_workflow
```

## Runs

Trigger an existing deployed workflow:

```bash
czar run start my_workflow
```

Trigger a deployed workflow with ad-hoc run inputs:

```bash
czar run start my_workflow --input date=2024-01-15 --input limit=5
```

Current run management commands:

```bash
czar run list
czar run list my_workflow
czar run list --status failed
czar run list --limit 10
czar run status <run_id>
czar run logs <run_id>
czar run logs <run_id> --follow
czar run logs <run_id> --task extract
czar run cancel <run_id>
czar run retry <run_id>
czar run retry <run_id> --task transform
```

Update schedule state without redeploying workflow YAML:

```bash
czar workflow update my_workflow --schedule-enabled=false
czar workflow update my_workflow --schedule-enabled=true
```

Manually trigger a deployed workflow by name:

```bash
czar trigger my_workflow
```

## Validation And Schema Inspection

Validate workflow YAML against the current compiler and Python contracts:

```bash
czar validate-workflow workflow.yaml
```

Inspect a Python task signature:

```bash
czar schema export tasks.py
```

## Scheduling

Schedules are defined in workflow YAML:

<!-- doc-snippet: path=examples/reference/future-operations/01-schedule.yaml sha256=5e59b14a2176dee38f1bba54e886305fbb1ab3786f7fa859bffffcfb202e0814 lines=1-5 -->
```yaml
name: my_etl
schedule: "0 2 * * *"  # Daily at 2am UTC

tasks:
  # ...
```

Current behavior:

- schedules are stored when you run `czar workflow deploy`
- scheduled runs are created by the scheduler while `czar serve` is running
- `czar workflow update <name> --schedule-enabled=<bool>` toggles whether the stored schedule is active
- enabling requires the workflow to already have a `schedule:` configured

## Current Limits

- `czar run start --input NAME=VALUE` stores run-scoped inputs and exposes them through existing `ref: vars.NAME` bindings at execution time
- `--input` values are parsed as JSON when possible; otherwise they are treated as strings
- trigger-time inputs do not recompile deployed file paths or `${{ vars.NAME }}` compile-time interpolation
- filtering and selection live on `czar run <workflow.yaml>` and `czar trigger`, not on every subcommand
