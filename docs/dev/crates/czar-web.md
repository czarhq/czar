# czar-web

_Updated at: 2026-05-22_

Axum-based web UI/API backed by the primary database. It reads workflows/runs/tasks and triggers new runs against the DB-backed scheduler.

## Owns

- Web API routes for listing workflows, runs, and tasks
- Run trigger endpoint that creates `runs` rows in Postgres
- Workflow creation endpoint that accepts YAML and compiles DAGs
- Web server bootstrapping

## Notes

- Does not run the scheduler loop; it expects `czar run` (or another scheduler host) to be running.

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/czar-web/Cargo.toml) | Web crate manifest with backend storage feature flags (postgres, sqlite, d1). | 2026-03-16 | 7b79983 |
| [src/api.rs](../../../crates/czar-web/src/api.rs) | `ApiServer` type, `StateUpdate` broadcast events, and `build_router` wiring all API and websocket routes. | 2026-03-16 | 0f4dbd8 |
| [src/api_helpers.rs](../../../crates/czar-web/src/api_helpers.rs) | Shared API helpers (time formatting, not-found detection). | 2026-02-06 | 84c8138 |
| [src/api_realtime.rs](../../../crates/czar-web/src/api_realtime.rs) | Realtime websocket endpoints plus SPA shell responses for non-API routes. | 2026-03-14 | fe7d250 |
| [src/api_realtime/tests.rs](../../../crates/czar-web/src/api_realtime/tests.rs) | Unit tests for realtime sinks and broadcast pump behavior. | 2026-03-14 | 692b764 |
| [src/api_routes.rs](../../../crates/czar-web/src/api_routes.rs) | Paginated list endpoints for runs and workflows, workflow detail with task states and YAML, and the workflow task-states endpoint. | 2026-03-12 | a0fb651 |
| [src/api_routes/api_routes_control.rs](../../../crates/czar-web/src/api_routes/api_routes_control.rs) | Run/task control endpoints (cancel/pause/resume) and workflow scheduling updates. | 2026-03-03 | b4d56d6 |
| [src/api_routes/api_routes_start_run.rs](../../../crates/czar-web/src/api_routes/api_routes_start_run.rs) | Start-run endpoint with flow/group/task selection resolution. | 2026-03-03 | 5652fa8 |
| [src/handlers.rs](../../../crates/czar-web/src/handlers.rs) | Web-server startup helpers, route assembly, workflow bootstrap support, and long-running service orchestration. | 2026-03-15 | 779c9a3 |
| [src/lib.rs](../../../crates/czar-web/src/lib.rs) | Library module exports for API, handlers, workflow helpers, and UI bundle resolution. | 2026-03-14 | 229b3b0 |
| [src/main.rs](../../../crates/czar-web/src/main.rs) | Binary entrypoint for the standalone web server, including backend bootstrap and optional workflow-definition loading. | 2026-03-15 | 9dba911 |
| [src/main/bootstrap.rs](../../../crates/czar-web/src/main/bootstrap.rs) | Backend/bootstrap helpers for CLI config parsing, backend selection, and database initialization including D1. | 2026-03-14 | 49b3a6f |
| [src/main/tests.rs](../../../crates/czar-web/src/main/tests.rs) | Unit tests for web startup config resolution, backend selection, and server boot edge cases. | 2026-03-15 | d240d9c |
| [src/ui_bundle.rs](../../../crates/czar-web/src/ui_bundle.rs) | React UI bundle path resolution and HTML shell loading for dev and packaged deployments. | 2026-03-15 | e4192be |
| [src/workflow_groups.rs](../../../crates/czar-web/src/workflow_groups.rs) | Workflow flow/group expansion and snapshot task-building helpers for API routes. | 2026-03-03 | 16ff789 |
| [src/workflow_tasks.rs](../../../crates/czar-web/src/workflow_tasks.rs) | Build workflow task rows from compiled workflows. | 2026-02-08 | 8ae3c4e |
| [tests/api_db_error_paths_test.rs](../../../crates/czar-web/tests/api_db_error_paths_test.rs) | Integration tests for DB failure/error branches across API handlers. | 2026-03-12 | 4d9b2e8 |
| [tests/api_endpoints_test.rs](../../../crates/czar-web/tests/api_endpoints_test.rs) | Integration tests for HTTP API endpoints. | 2026-03-04 | 2c176c5 |
| [tests/api_error_paths_test.rs](../../../crates/czar-web/tests/api_error_paths_test.rs) | Integration tests for HTTP API error paths: invalid YAML, missing resources, conflict states, invalid UUIDs, and case-insensitive search. | 2026-03-12 | 7ee73c5 |
| [tests/api_realtime_test.rs](../../../crates/czar-web/tests/api_realtime_test.rs) | Integration tests for SPA asset serving and websocket realtime updates. | 2026-03-14 | e0d22e0 |
| [tests/api_task_states_test.rs](../../../crates/czar-web/tests/api_task_states_test.rs) | Integration test verifying the `/api/workflows/{name}/task-states` endpoint returns correct persisted state, output, and timestamps. | 2026-03-12 | 92ff8fc |
| [tests/test_snapshot_tasks.rs](../../../crates/czar-web/tests/test_snapshot_tasks.rs) | Integration test verifying that a workflow created via API gets a snapshot, runs capture the snapshot ID, and tasks can be built from it. | 2026-03-12 | 5d7ada5 |
