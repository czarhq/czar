# czar-core

_Updated at: 2026-05-22_

The base crate: models, traits, and the scheduler loop. Everything else builds on this.

## Owns

- Domain models (`Workflow`, `Run`, `Task`, `ExecutorType`)
- Status enums and helpers for string conversion
- Core traits: `Database`, `Executor`, `ExecutorManager`
- Scheduler loop (`Scheduler`) and `OrchestratorConfig`
- `StatusUpdate` message type
- YAML workflow definitions and compilation utilities

## Extension Points

- Implement [`Database`](../../../crates/czar-core/src/database.rs) in [czar-store](czar-store.md) (storage backends).
- Implement [`Executor`](../../../crates/czar-core/src/executor.rs) in [czar-executors](czar-executors.md) (execution backends).

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/czar-core/Cargo.toml) | Core crate manifest with scheduler, storage-contract, and optional SQLx dependencies. | 2026-03-14 | 9f546f7 |
| [src/compiled.rs](../../../crates/czar-core/src/compiled.rs) | Workflow compilation pipeline that resolves executor-specific task metadata and builds persisted workflow-task rows. | 2026-03-14 | 8ea8030 |
| [src/compiled/tests.rs](../../../crates/czar-core/src/compiled/tests.rs) | Unit tests for compiled workflow task generation, executor-specific params, and path resolution. | 2026-03-14 | 60d8a10 |
| [src/compiled/tests/python.rs](../../../crates/czar-core/src/compiled/tests/python.rs) | Python-specific compilation and signature-introspection tests. | 2026-03-14 | 9744cbb |
| [src/config.rs](../../../crates/czar-core/src/config.rs) | Scheduler configuration defaults and tuning knobs. | 2026-03-12 | 684990b |
| [src/config/tests.rs](../../../crates/czar-core/src/config/tests.rs) | Tests OrchestratorConfig defaults, profiles, and environment variable overrides. | 2026-03-12 | 87d0651 |
| [src/database.rs](../../../crates/czar-core/src/database.rs) | Repository trait contracts and DTOs for workflows, runs, tasks, schedules, deferred jobs, and the composite `Database` interface. | 2026-03-12 | 8c58190 |
| [src/error.rs](../../../crates/czar-core/src/error.rs) | Error types for workflow loading and execution. | 2026-02-09 | 3da2159 |
| [src/events.rs](../../../crates/czar-core/src/events.rs) | Scheduler event types for broadcasting run/task lifecycle changes. | 2026-03-13 | 469dfe1 |
| [src/executor.rs](../../../crates/czar-core/src/executor.rs) | Shared executor trait and status-update channel contract used by runtime backends. | 2026-03-14 | 8fa3cf7 |
| [src/executor_manager.rs](../../../crates/czar-core/src/executor_manager.rs) | Executor manager trait for per-task executors. | 2026-02-06 | d1d5a9a |
| [src/job_tracker.rs](../../../crates/czar-core/src/job_tracker.rs) | Deferrable job tracker implementations for external systems and custom HTTP polling. | 2026-02-09 | 8a4b9f1 |
| [src/job_tracker/tests.rs](../../../crates/czar-core/src/job_tracker/tests.rs) | Unit tests for custom HTTP tracker validation behavior. | 2026-03-03 | fe88f47 |
| [src/lib.rs](../../../crates/czar-core/src/lib.rs) | Top-level module wiring and public exports for the core orchestration runtime. | 2026-03-14 | c59ac60 |
| [src/models.rs](../../../crates/czar-core/src/models.rs) | Core persisted domain models for workflows, runs, tasks, snapshots, schedules, and deferred jobs. | 2026-03-14 | 0b3f98e |
| [src/models/tests.rs](../../../crates/czar-core/src/models/tests.rs) | Unit tests for model helpers, executor parsing, and workflow-definition normalization behavior. | 2026-03-14 | a1b3299 |
| [src/python_runtime.rs](../../../crates/czar-core/src/python_runtime.rs) | Shared helpers for resolving workflow or task-specific Python project roots and constructing uv-based Python commands. | 2026-03-12 | b0ac67f |
| [src/schedule_processor.rs](../../../crates/czar-core/src/schedule_processor.rs) | Cron schedule processing and run triggering. | 2026-02-08 | e15adf1 |
| [src/scheduler.rs](../../../crates/czar-core/src/scheduler.rs) | Main scheduler loop, polling orchestration, and job-tracker registration. | 2026-03-14 | 7f3bebd |
| [src/scheduler/processing.rs](../../../crates/czar-core/src/scheduler/processing.rs) | Scheduler handlers for pending runs/tasks and scheduled triggers, including snapshot/selection task creation and dispatch preparation. | 2026-03-12 | 204223e |
| [src/scheduler/status_processing.rs](../../../crates/czar-core/src/scheduler/status_processing.rs) | Status-update processing pipeline for logs/output/progress, retry/timeout handling, task-state persistence, dependency-failure propagation, and run completion. | 2026-03-12 | 0077090 |
| [src/services/mod.rs](../../../crates/czar-core/src/services/mod.rs) | Service module exports for workflow application paths. | 2026-03-14 | 5be2eec |
| [src/services/workflow_yaml.rs](../../../crates/czar-core/src/services/workflow_yaml.rs) | Service that parses, validates, compiles, and applies a YAML workflow to the database, creating or reusing a content-hash snapshot. | 2026-03-12 | a0dd480 |
| [src/task_execution.rs](../../../crates/czar-core/src/task_execution.rs) | Task dispatch helpers for binding resolution, retry metadata, upstream-output lookup, and run-task materialization. | 2026-03-14 | 1b796c9 |
| [src/task_execution/tests.rs](../../../crates/czar-core/src/task_execution/tests.rs) | Test fixtures (mock executor/manager) and unit tests for task dispatch, binding resolution, and persisted-state fallback for missing in-run dependencies. | 2026-03-12 | d351bed |
| [src/task_execution/tests/execute_task.rs](../../../crates/czar-core/src/task_execution/tests/execute_task.rs) | Tests task execution with validation, dependency checks, and binding resolution. | 2026-03-12 | 45e4c96 |
| [src/task_execution/tests/selection.rs](../../../crates/czar-core/src/task_execution/tests/selection.rs) | Task-selection and run-task filtering tests. | 2026-03-04 | 41a9142 |
| [src/triggerer.rs](../../../crates/czar-core/src/triggerer.rs) | Triggerer loop that polls deferred jobs and reports completion back to the scheduler. | 2026-03-12 | 38eea23 |
| [src/triggerer/tests.rs](../../../crates/czar-core/src/triggerer/tests.rs) | Tests TriggererConfig defaults and JobCompletionNotification. | 2026-03-12 | de4f8b5 |
| [src/types.rs](../../../crates/czar-core/src/types.rs) | Legacy/shared runtime DTOs such as `Run`, `TaskRun`, and `TaskSpec`. | 2026-03-14 | 2c3615e |
| [src/types/tests.rs](../../../crates/czar-core/src/types/tests.rs) | Tests TaskSpec, Run initialization, and workflow/compiled task creation, including compiled-task Python path metadata. | 2026-03-12 | 3408eab |
| [src/worker_client.rs](../../../crates/czar-core/src/worker_client.rs) | HTTP client used by the scheduler to call remote czar-worker compile and execute endpoints. | 2026-03-14 | 971e568 |
| [src/worker_client/tests.rs](../../../crates/czar-core/src/worker_client/tests.rs) | Unit tests for worker-client request shaping, error handling, and response decoding. | 2026-03-14 | 32fc459 |
| [src/workflow.rs](../../../crates/czar-core/src/workflow.rs) | Workflow schema types, parsing entrypoints, and validation/definition module wiring. | 2026-03-14 | 531e0e2 |
| [src/workflow/workflow_definition_impl.rs](../../../crates/czar-core/src/workflow/workflow_definition_impl.rs) | Flow/group expansion and workflow-definition normalization helpers. | 2026-03-14 | 02190d5 |
| [src/workflow/workflow_tests_core.rs](../../../crates/czar-core/src/workflow/workflow_tests_core.rs) | Core workflow parsing and validation regression tests. | 2026-03-14 | eb07c71 |
| [src/workflow/workflow_tests_definition.rs](../../../crates/czar-core/src/workflow/workflow_tests_definition.rs) | Workflow-definition parsing/selection tests. | 2026-03-03 | d8a88d9 |
| [src/workflow/workflow_tests_types.rs](../../../crates/czar-core/src/workflow/workflow_tests_types.rs) | Tests task type schema validation: accepted primitives and arrays, rejected unknown/empty/multi-element/numeric values, and nested object acceptance. | 2026-03-12 | 8081c05 |
| [src/workflow/workflow_validation_impl.rs](../../../crates/czar-core/src/workflow/workflow_validation_impl.rs) | Workflow validation rules for executor requirements, dependencies, and type contracts. | 2026-03-14 | ec3094f |
| [src/workflow_source.rs](../../../crates/czar-core/src/workflow_source.rs) | Loads workflow YAML sources, including generated descriptors, and enforces uv/pyproject execution for Python generators. | 2026-03-12 | ad9e446 |
| [src/workflow_source/tests.rs](../../../crates/czar-core/src/workflow_source/tests.rs) | Tests direct YAML loading, generator descriptors, caching/refresh behavior, and required pyproject enforcement for Python generators. | 2026-03-12 | e92761b |
| [tests/compiled_workflow_test.rs](../../../crates/czar-tests/tests/core/compiled_workflow_test.rs) | Integration tests for workflow compilation success and missing-file error behavior. | 2026-02-12 | fd98831 |
| [tests/dag_execution_test.rs](../../../crates/czar-tests/tests/core/dag_execution_test.rs) | DAG execution order/performance integration test. | 2026-02-08 | 0a5de38 |
| [tests/deferrables_test.rs](../../../crates/czar-tests/tests/core/deferrables_test.rs) | Integration tests for deferrable task lifecycle handling. | 2026-02-07 | e13ff26 |
| [tests/pause_resume_test.rs](../../../crates/czar-tests/tests/core/pause_resume_test.rs) | Pause/resume scheduler integration tests. | 2026-02-08 | 03f0705 |
| [tests/schedule_processor_test.rs](../../../crates/czar-tests/tests/core/schedule_processor_test.rs) | Integration tests for scheduled workflow processing and trigger time updates. | 2026-02-08 | afe4504 |
| [tests/scheduler_channel_behavior_test.rs](../../../crates/czar-tests/tests/core/scheduler_channel_behavior_test.rs) | Scheduler integration tests for status/job-completion channel behavior, error resilience, timeout edge cases, and deferred creation failures. | 2026-03-12 | e3b633b |
| [tests/scheduler_processing_behavior_test.rs](../../../crates/czar-tests/tests/core/scheduler_processing_behavior_test.rs) | Integration coverage for scheduler processing paths: pending-run initialization, dispatch retries, deferred payload handling, selection behavior, and failure fallbacks. | 2026-03-12 | 52367e0 |
| [tests/scheduler_processing_behavior_test/postgres_faults.rs](../../../crates/czar-tests/tests/core/scheduler_processing_behavior_test/postgres_faults.rs) | Toxiproxy-backed scheduler integration tests that route the scheduler's Postgres connection through a fault-injection proxy and currently cover full DB disconnect plus a deterministic 1500 ms latency spike. See [docs/dev/postgres-fault-injection.md](../postgres-fault-injection.md). | 2026-04-05 | pending |
| [tests/scheduler_processing_behavior_test/persisted_selection.rs](../../../crates/czar-tests/tests/core/scheduler_processing_behavior_test/persisted_selection.rs) | Scheduler integration test for executing selected tasks with missing in-run dependencies via persisted task state outputs. | 2026-03-04 | 62b03f1 |
| [tests/scheduler_retry_fallback_test.rs](../../../crates/czar-tests/tests/core/scheduler_retry_fallback_test.rs) | Integration test for the retry-reset failure fallback path that still marks tasks/runs as failed. | 2026-02-08 | f0d6130 |
| [tests/scheduler_startup_test.rs](../../../crates/czar-tests/tests/core/scheduler_startup_test.rs) | Integration tests for scheduler startup behavior and triggerer initialization path. | 2026-02-08 | 19ffd91 |
| [tests/scheduler_upstream_output_error_test.rs](../../../crates/czar-tests/tests/core/scheduler_upstream_output_error_test.rs) | Integration test ensuring downstream execution proceeds when upstream output decoding fails. | 2026-03-12 | 48742d0 |
| [tests/sqlite_deferred_jobs_test.rs](../../../crates/czar-tests/tests/core/sqlite_deferred_jobs_test.rs) | SQLite integration tests for deferred job lifecycle and cancellation behavior. | 2026-02-08 | 725838d |
| [tests/triggerer_behavior_test.rs](../../../crates/czar-tests/tests/core/triggerer_behavior_test.rs) | Integration tests for triggerer polling outcomes: running, completion, failure, timeout, and missing tracker behavior. | 2026-02-08 | f7ed090 |
| [tests/type_extraction_test.rs](../../../crates/czar-tests/tests/core/type_extraction_test.rs) | Integration tests for Python task signature extraction and typed/untyped compilation outcomes inside project-scoped roots. | 2026-03-12 | 673c29e |
