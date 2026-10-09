# czar-store-d1

_Updated at: 2026-05-22_

Cloudflare D1 backend for Czar. Implements the [`czar-core::Database`](../../../crates/czar-core/src/database.rs) traits against D1's HTTP API.

## Owns

- D1 client and request/response decoding
- D1-backed implementations of workflow, run, task, schedule, and deferred-job repositories
- D1 migration runner

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/czar-store-d1/Cargo.toml) | D1 backend crate manifest and dependency wiring. | 2026-03-14 | dc219b1 |
| [migrations/001_init.sql](../../../crates/czar-store-d1/migrations/001_init.sql) | Initial Cloudflare D1 schema mirroring Czar workflow/run/task tables. | 2026-03-14 | 04e938b |
| [src/client.rs](../../../crates/czar-store-d1/src/client.rs) | Thin D1 HTTP client and typed query parameter/result handling. | 2026-03-14 | f290ca5 |
| [src/database/helpers.rs](../../../crates/czar-store-d1/src/database/helpers.rs) | Shared D1 parameter encoding and row-scalar decoding helpers. | 2026-03-14 | f776f27 |
| [src/database/mod.rs](../../../crates/czar-store-d1/src/database/mod.rs) | Top-level D1 database implementation, repository trait wiring, migrations, and shared query helpers. | 2026-03-15 | db129c6 |
| [src/database/runs.rs](../../../crates/czar-store-d1/src/database/runs.rs) | D1-backed run repository queries, status updates, cancellation, and pagination helpers. | 2026-03-14 | 2cd2ffc |
| [src/database/schedule_deferred.rs](../../../crates/czar-store-d1/src/database/schedule_deferred.rs) | D1-backed schedule and deferred-job repository implementations. | 2026-03-14 | 21a718d |
| [src/database/task_queries.rs](../../../crates/czar-store-d1/src/database/task_queries.rs) | D1 task query helpers for pending dispatch, outputs, retry metadata, task identity, and dependency failure handling. | 2026-03-14 | 73854d7 |
| [src/database/task_repo.rs](../../../crates/czar-store-d1/src/database/task_repo.rs) | `TaskRepository` implementation that delegates task CRUD and lifecycle operations to the D1 helpers. | 2026-03-14 | 6291772 |
| [src/database/tasks_core.rs](../../../crates/czar-store-d1/src/database/tasks_core.rs) | Core D1 task lifecycle queries for creation, status updates, logs, output, progress, and retry resets. | 2026-03-14 | c469381 |
| [src/database/workflows.rs](../../../crates/czar-store-d1/src/database/workflows.rs) | D1-backed workflow and snapshot repository implementations. | 2026-03-14 | 4c935f3 |
| [src/decode.rs](../../../crates/czar-store-d1/src/decode.rs) | Row decoders mapping D1 JSON payloads into Czar core models. | 2026-03-14 | 66eb625 |
| [src/lib.rs](../../../crates/czar-store-d1/src/lib.rs) | Public crate entrypoint exporting `D1Database` and `D1Config`. | 2026-03-14 | b8c4257 |
| [src/migrations.rs](../../../crates/czar-store-d1/src/migrations.rs) | D1 migration runner and migration-file loader. | 2026-03-14 | 97d9379 |
| [src/tests/lifecycle.rs](../../../crates/czar-store-d1/src/tests/lifecycle.rs) | Mock-D1 lifecycle smoke tests covering migrations, workflow snapshots, runs, tasks, and cancellation behavior. | 2026-03-15 | 8b350a4 |
| [src/tests/mod.rs](../../../crates/czar-store-d1/src/tests/mod.rs) | Test module wiring for the D1 backend unit/integration suite. | 2026-03-15 | 8399e70 |
| [src/tests/scheduling.rs](../../../crates/czar-store-d1/src/tests/scheduling.rs) | Mock-D1 tests for schedules, deferred jobs, retry windows, and dependency-failure task visibility. | 2026-03-15 | 812de48 |
| [src/tests/support.rs](../../../crates/czar-store-d1/src/tests/support.rs) | Shared mock D1 HTTP server and SQLite-backed query harness for backend tests. | 2026-03-15 | 51a8b59 |
| [tests/support/mod.rs](../../../crates/czar-store-d1/tests/support/mod.rs) | Shared helpers for Wrangler-backed D1 integration tests, including process lifecycle and readiness checks. | 2026-03-15 | f6250fb |
| [tests/wrangler_dev.rs](../../../crates/czar-store-d1/tests/wrangler_dev.rs) | Ignored integration test that exercises the D1 backend against a local Wrangler dev harness. | 2026-03-15 | 33e9ebf |
| [tests/wrangler_full_dev.rs](../../../crates/czar-store-d1/tests/wrangler_full_dev.rs) | Ignored end-to-end local Cloudflare test covering Worker, container app, and D1 together under Wrangler dev. | 2026-03-15 | 66efada |
