# Czar Crates

This directory contains the Rust workspace crates for Czar.

## Crate Overview

### Core

| Crate | Description |
|---|---|
| [`czar-core`](czar-core/) | Orchestration engine: workflow scheduling, DAG execution, task lifecycle |
| [`czar-store`](czar-store/) | State facade, backend feature wiring, object-store helpers |
| [`czar-store-file`](czar-store-file/) | File-backed state adapter |
| [`czar-store-sqlite`](czar-store-sqlite/) | SQLite state adapter |
| [`czar-store-postgres`](czar-store-postgres/) | Postgres state adapter |
| [`czar-store-d1`](czar-store-d1/) | Cloudflare D1 database adapter |
| [`czar-executors`](czar-executors/) | Task executors: process, library (`.so`/`.dylib`), wasm, Cloud Run, remote worker |

### SDKs

| Crate | Description |
|---|---|
| [`czar`](czar/) | The Rust task SDK: `#[task]`, `Context`, task I/O helpers, C ABI and Wasm entry points, deferrables |
| [`czar-macros`](czar-macros/) | Procedural macros (`#[task]`) for defining Czar tasks |

### User Interfaces

| Crate | Description |
|---|---|
| [`czar-cli`](czar-cli/) | `czar` binary: CLI commands, scheduler host, `perf-test` and `yaml-validate` tools |
| [`czar-tui`](czar-tui/) | Terminal UI for monitoring runs and tasks |
| [`czar-web`](czar-web/) | REST API server (Axum) |
| [`czar-web-react`](czar-web-react/) | React dashboard frontend (TypeScript, WebSocket, SVG DAG graphs) |

### Other

| Crate | Description |
|---|---|
| [`czar-worker`](czar-worker/) | Remote worker executor for distributed task dispatch |

## Development

```bash
# Build everything
cargo build

# Run tests
cargo test

# Frontend (czar-web-react)
cd crates/czar-web-react && pnpm install --frozen-lockfile && pnpm run dev
```

See [`docs/dev/architecture/building-blocks.md`](../docs/dev/architecture/building-blocks.md) for a deeper look at how the crates fit together.
