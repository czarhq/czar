# czar-worker

_Updated at: 2026-09-29_

Stateless HTTP server binary that compiles a packaged workflow and executes its tasks on demand.

## Overview

`czar-worker` runs alongside a packaged workflow; the workflow source and any task assets are expected to exist on the local filesystem when the binary starts. The orchestration layer (scheduler/czar-web) drives it by sending HTTP requests; the worker handles them synchronously and returns results.

## Startup

```
czar-worker --workflow-path <path> --working-dir <dir> [--addr <addr>] [--auth-token <token>]
```

- `--workflow-path`: path to the workflow source file (default: `workflow.yaml`)
- `--working-dir`: base directory used when resolving task assets (default: `.`)
- `--addr`: bind address (default: `127.0.0.1:8081`)
- `--auth-token`: require `Authorization: Bearer <token>` on `/compile` and `/execute` (env: `CZAR_WORKER_AUTH_TOKEN`)

## Endpoints

- `POST /compile`: read the workflow source from disk, validate it, return serialised task definitions
- `POST /execute`: execute one task and return its output or error
- `POST /health`: liveness check

## Execution Backends

The execute endpoint dispatches to one of several backends depending on the task's executor type: `process`, `python`, `library`, `wasm`.

Each backend resolves any path parameters relative to the workflow root before handing off to the corresponding `czar-executors` implementation.

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/czar-worker/Cargo.toml) | Worker crate manifest and executor/backend feature wiring. | 2026-03-14 | 2516013 |
| [src/execute/mod.rs](../../../crates/czar-worker/src/execute/mod.rs) | Execute endpoint entrypoint, request dispatch, and executor module wiring. | 2026-03-14 | 1e8fba5 |
| [src/execute/tests.rs](../../../crates/czar-worker/src/execute/tests.rs) | Handler-level tests for execute endpoint success, unsupported executors, and backend-specific path handling. | 2026-03-14 | fde9a89 |
| [src/handlers/compile.rs](../../../crates/czar-worker/src/handlers/compile.rs) | HTTP compile endpoint that loads a workflow from disk and returns compiled task definitions. | 2026-03-14 | ad2bca8 |
| [src/handlers/compile/tests.rs](../../../crates/czar-worker/src/handlers/compile/tests.rs) | Tests worker compile-handler success and validation/error paths, including Python generator and project-root requirements. | 2026-03-12 | 86419f3 |
| [src/handlers/execute.rs](../../../crates/czar-worker/src/handlers/execute.rs) | Worker execute endpoint and backend dispatch. | 2026-03-14 | working tree |
| [src/handlers/execute_request.rs](../../../crates/czar-worker/src/handlers/execute_request.rs) | Execute request/response payloads and requested-executor parsing for worker HTTP calls. | 2026-03-14 | working tree |
| [src/main.rs](../../../crates/czar-worker/src/main.rs) | Worker binary entrypoint that parses config and serves compile, execute, and health endpoints. | 2026-03-14 | 2385e81 |
| [src/main/tests/mod.rs](../../../crates/czar-worker/src/main/tests/mod.rs) | Worker main test-module wiring. | 2026-03-12 | 041c98b |
