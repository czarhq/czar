# czar-cli

_Updated at: 2026-05-22_

The main host process for Czar. It owns the CLI surface and starts the scheduler loop.

## Owns

- CLI commands for workflows, runs, tasks, and DB init
- `load` and `run` as the public YAML-first workflow entrypoints
- Scheduler host wiring ([`czar-core::Scheduler`](../../../crates/czar-core/src/scheduler.rs) + [czar-store](czar-store.md) + [czar-executors](czar-executors.md))
- YAML workflow compilation into `workflow_tasks`
- Postgres schema and migrations via `czar-store-postgres`
- Perf test binary and configs

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [.dockerignore](../../../.dockerignore) | Excludes local artifacts from the Docker build context. | 2026-02-06 | e773e90 |
| [Cargo.toml](../../../crates/czar-cli/Cargo.toml) | CLI crate manifest with backend feature selection, D1 support, and serve/runtime dependency wiring. | 2026-03-15 | 52217d1 |
| [Dockerfile](../../../crates/czar-cli/Dockerfile) | Container image build for running the czar CLI/scheduler. | 2026-02-06 | 29c76d0 |
| [README.md](../../../crates/czar-cli/README.md) | User-facing CLI usage guide with command examples and selector syntax reference. | 2026-03-12 | 5d8523e |
| [docker-compose.yml](../../../crates/czar-cli/docker-compose.yml) | Local Postgres service definition for development and tests. | 2026-02-06 | 73415d6 |
| [justfile](../../../crates/czar-cli/justfile) | Just tasks for running, testing, and performance workflows. | 2026-02-06 | 138d510 |
| [state migrations](../../../crates/czar-store-postgres/migrations/) | Postgres schema migrations used by CLI database initialization. | 2026-03-12 | working tree |
| [perf-configs/heavy.yaml](../../../crates/czar-cli/perf-configs/heavy.yaml) | Load-test configuration for heavy stress runs. | 2026-02-06 | 0563280 |
| [perf-configs/latency.yaml](../../../crates/czar-cli/perf-configs/latency.yaml) | Load-test configuration for latency-focused runs. | 2026-02-06 | 7068a4c |
| [perf-configs/memory.yaml](../../../crates/czar-cli/perf-configs/memory.yaml) | Load-test configuration for memory-focused runs. | 2026-02-06 | 4d94588 |
| [perf-configs/quick.yaml](../../../crates/czar-cli/perf-configs/quick.yaml) | Load-test configuration for quick smoke runs. | 2026-02-06 | 6bb5184 |
| [perf-configs/standard.yaml](../../../crates/czar-cli/perf-configs/standard.yaml) | Load-test configuration for standard runs. | 2026-02-06 | 76d5f60 |
| [schema.sql](../../../crates/czar-cli/schema.sql) | Generated schema snapshot after applying all migrations. | 2026-02-06 | 9905605 |
| [scripts/dump-schema.sh](../../../crates/czar-cli/scripts/dump-schema.sh) | Generates `schema.sql` via pg_dump after migrations. | 2026-02-06 | b698ac4 |
| [scripts/test-e2e.sh](../../../crates/czar-cli/scripts/test-e2e.sh) | End-to-end CLI test that triggers and inspects a run. | 2026-02-06 | bd553d1 |
| [scripts/test-load.sh](../../../crates/czar-cli/scripts/test-load.sh) | Simple load test that triggers many workflows. | 2026-02-06 | 844ff75 |
| [src/bin/perf-test.rs](../../../crates/czar-cli/src/bin/perf-test.rs) | Performance test CLI that generates load, boots a scheduler, and summarizes throughput/resource metrics. | 2026-03-15 | d6981f0 |
| [src/bin/perf-test/tests.rs](../../../crates/czar-cli/src/bin/perf-test/tests.rs) | Tests performance config deserialization and scheduler metrics parsing. | 2026-03-12 | ea7b63f |
| [src/bin/perf_test/integration_tests.rs](../../../crates/czar-cli/src/bin/perf_test/integration_tests.rs) | Integration-style perf-test coverage using a mock `czar` binary and controlled config. | 2026-02-12 | 39e874f |
| [src/bin/perf_test/perf_metrics.rs](../../../crates/czar-cli/src/bin/perf_test/perf_metrics.rs) | Helpers for parsing and summarizing scheduler metrics in perf-test output. | 2026-03-12 | 761a1d4 |
| [src/bin/perf_test/perf_metrics/tests.rs](../../../crates/czar-cli/src/bin/perf_test/perf_metrics/tests.rs) | Tests metrics parsing, summarization, and formatting for performance monitoring. | 2026-03-12 | 4a2daa6 |
| [src/bin/perf_test/perf_support.rs](../../../crates/czar-cli/src/bin/perf_test/perf_support.rs) | Shared perf-test helpers for config loading, temp script generation, scheduler config writing, and throughput reporting. | 2026-03-15 | d8b6bc7 |
| [src/bin/perf_test/perf_support/tests.rs](../../../crates/czar-cli/src/bin/perf_test/perf_support/tests.rs) | Unit tests for perf-test support helpers, including temp script generation and environment-driven defaults. | 2026-03-15 | 5af3551 |
| [src/bootstrap.rs](../../../crates/czar-cli/src/bootstrap.rs) | Shared CLI bootstrap helpers for backend selection, database initialization, and D1 config resolution. | 2026-03-15 | 209ee5a |
| [src/commands/create_workflow_yaml.rs](../../../crates/czar-cli/src/commands/create_workflow_yaml.rs) | Implements `czar load`: loads workflow source text and applies YAML through the workflow service with create-only behavior. | 2026-03-12 | 71a7012 |
| [src/commands/create_workflow_yaml/tests.rs](../../../crates/czar-cli/src/commands/create_workflow_yaml/tests.rs) | Tests YAML workflow apply behavior, including workflow/task creation and root-path fallback handling. | 2026-03-12 | a358693 |
| [src/commands/delete_workflow.rs](../../../crates/czar-cli/src/commands/delete_workflow.rs) | Implements `czar delete-workflow`. | 2026-03-12 | bf02014 |
| [src/commands/delete_workflow/tests.rs](../../../crates/czar-cli/src/commands/delete_workflow/tests.rs) | Tests DeleteWorkflow command removes workflows from the database. | 2026-03-12 | ce84451 |
| [src/commands/execute.rs](../../../crates/czar-cli/src/commands/execute.rs) | Internal helper used by `czar run` to apply workflow YAML, trigger a run, and wait for completion. | 2026-03-12 | 708cbf9 |
| [src/commands/execute/tests.rs](../../../crates/czar-cli/src/commands/execute/tests.rs) | Tests the internal run path for success, failure, selector, timeout, and workflow-replacement behavior. | 2026-03-12 | 7ee62e8 |
| [src/commands/init.rs](../../../crates/czar-cli/src/commands/init.rs) | Implements `czar init` migrations. | 2026-03-12 | cf379d9 |
| [src/commands/init/tests.rs](../../../crates/czar-cli/src/commands/init/tests.rs) | Tests Init command runs database migrations. | 2026-03-12 | 21a60d4 |
| [src/commands/list_workflows.rs](../../../crates/czar-cli/src/commands/list_workflows.rs) | Implements `czar list-workflows`. | 2026-03-12 | 17abc4d |
| [src/commands/list_workflows/tests.rs](../../../crates/czar-cli/src/commands/list_workflows/tests.rs) | Tests ListWorkflows command with empty and populated database. | 2026-03-12 | 2257c69 |
| [src/commands/mod.rs](../../../crates/czar-cli/src/commands/mod.rs) | Command registry wiring: exports command modules/types, defines clap subcommand ordering/aliases, and API-routing classification. | 2026-03-12 | 687acb7 |
| [src/commands/run.rs](../../../crates/czar-cli/src/commands/run.rs) | Implements `czar run`: applies a workflow YAML locally, supports `--var` overrides plus flow/selector filtering, and waits for terminal status. | 2026-03-12 | e48121d |
| [src/commands/run/tests.rs](../../../crates/czar-cli/src/commands/run/tests.rs) | Tests run-command success, missing-file handling, and selector-based subset execution paths. | 2026-03-12 | 6489903 |
| [src/commands/logs.rs](../../../crates/czar-cli/src/commands/logs.rs) | Implements `czar logs`: prints captured task logs and error text for a run, optionally filtered to one task. | 2026-03-25 | working tree |
| [src/commands/selection_spec.rs](../../../crates/czar-cli/src/commands/selection_spec.rs) | Parses and expands CLI task selector syntax into exact/upstream/downstream task selections. | 2026-03-12 | 81c0b5d |
| [src/commands/serve.rs](../../../crates/czar-cli/src/commands/serve.rs) | Implements `czar serve`, including web startup, scheduler/runtime bootstrapping, and optional workflow bootstrap on launch. | 2026-03-15 | d182830 |
| [src/commands/serve/tests.rs](../../../crates/czar-cli/src/commands/serve/tests.rs) | Tests Serve command configuration and web server startup. | 2026-03-12 | d3f7c3e |
| [src/commands/status.rs](../../../crates/czar-cli/src/commands/status.rs) | Implements `czar status`. | 2026-03-12 | 7d9115f |
| [src/commands/status/tests.rs](../../../crates/czar-cli/src/commands/status/tests.rs) | Tests Status command querying runs by ID, workflow, or all. | 2026-03-12 | ff42342 |
| [src/commands/tasks.rs](../../../crates/czar-cli/src/commands/tasks.rs) | Implements `czar tasks`. | 2026-03-12 | 5d5f738 |
| [src/commands/tasks/tests.rs](../../../crates/czar-cli/src/commands/tasks/tests.rs) | Tests Tasks command listing empty and populated task lists. | 2026-03-12 | 6e56c1a |
| [src/commands/tests.rs](../../../crates/czar-cli/src/commands/tests.rs) | Command-level unit checks for API-routing classification (`uses_api`) and related wiring assumptions. | 2026-03-12 | 871ad34 |
| [src/commands/trigger.rs](../../../crates/czar-cli/src/commands/trigger.rs) | Implements `czar trigger`: creates a run for an existing workflow and stores optional flow/selector metadata without waiting. | 2026-03-12 | e37186f |
| [src/commands/trigger/tests.rs](../../../crates/czar-cli/src/commands/trigger/tests.rs) | Tests trigger-command run creation and stored selection payloads for flow and selector variants. | 2026-03-12 | 8871885 |
| [src/commands/validate_workflow.rs](../../../crates/czar-cli/src/commands/validate_workflow.rs) | Implements `czar validate-workflow`: loads a YAML file and validates it through the same `czar-core` workflow-definition validator used at runtime, reporting task count on success. | 2026-03-25 | working tree |
| [src/main.rs](../../../crates/czar-cli/src/main.rs) | CLI entrypoint that parses args, routes API-only commands, initializes the selected backend, and dispatches subcommands. | 2026-03-15 | b9aad78 |
| [src/main/tests/cli_bootstrap.rs](../../../crates/czar-cli/src/main/tests/cli_bootstrap.rs) | Tests CLI bootstrap helpers for backend selection, database URL defaults, env precedence, and D1 config resolution. | 2026-03-15 | a7d82ff |
| [src/main/tests/command_dispatch.rs](../../../crates/czar-cli/src/main/tests/command_dispatch.rs) | Tests main-command routing, API short-circuit behavior, dispatch coverage, and CLI error paths before DB init. | 2026-03-15 | 5871fb9 |
| [src/main/tests/mod.rs](../../../crates/czar-cli/src/main/tests/mod.rs) | Shared test-module wiring and helpers for the CLI main entrypoint suite. | 2026-03-15 | 8ad1bfb |
| [tests/main_entrypoint_test.rs](../../../crates/czar-cli/tests/main_entrypoint_test.rs) | Binary-level integration tests verifying `czar --help` succeeds and missing-file errors surface before any DB connection attempt. | 2026-03-12 | 480a202 |

## Notes

- Depends on [czar-core](czar-core.md), [czar-store](czar-store.md), and [czar-executors](czar-executors.md).
- CLI usage details live in [crates/czar-cli/README.md](../../../crates/czar-cli/README.md).
