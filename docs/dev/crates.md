# Crates

_Updated at: 2026-05-22_

This is a compressed map of the workspace so you do not have to read code to orient yourself. For deeper details, see per-crate notes under [docs/dev/crates/](crates/) (and [crates/czar-cli/README.md](../../crates/czar-cli/README.md) for the CLI).

## Workspace Map

| Crate             | What it owns                                          | Details                                                     |
| ----------------- | ----------------------------------------------------- | ----------------------------------------------------------- |
| **czar-cli**       | CLI, scheduler host process, migrations, perf tooling | [docs/dev/crates/czar-cli.md](crates/czar-cli.md)             |
| **czar-core**      | Domain models + traits + scheduler loop               | [docs/dev/crates/czar-core.md](crates/czar-core.md)           |
| **czar-store**     | State facade, backend feature wiring, object-store helpers | [docs/dev/crates/czar-store.md](crates/czar-store.md)      |
| **czar-store-file** | File-backed state implementation                        | [docs/dev/crates/czar-store-file.md](crates/czar-store-file.md) |
| **czar-store-sqlite** | SQLite state implementation                          | [docs/dev/crates/czar-store-sqlite.md](crates/czar-store-sqlite.md) |
| **czar-store-postgres** | Postgres state implementation                      | [docs/dev/crates/czar-store-postgres.md](crates/czar-store-postgres.md) |
| **czar-store-d1**  | Cloudflare D1 state implementation                      | [docs/dev/crates/czar-store-d1.md](crates/czar-store-d1.md)   |
| **czar-executors** | Executor backends + `ExecutorManager`                 | [docs/dev/crates/czar-executors.md](crates/czar-executors.md) |
| **czar-web**       | Axum UI/API backed by the DB                          | [docs/dev/crates/czar-web.md](crates/czar-web.md)             |
| **czar-worker**    | Worker process for compile/execute paths              | [docs/dev/crates/czar-worker.md](crates/czar-worker.md)       |
| **czar-tui**       | Terminal monitoring UI                                | [docs/dev/crates/czar-tui.md](crates/czar-tui.md)             |
| **czar-gcp**       | GCP integration helpers (Cloud Run support)           | [docs/dev/crates/czar-gcp.md](crates/czar-gcp.md)             |
| **czar**           | Lightweight task-authoring facade crate (`#[task]`, `Context`) | [docs/dev/crates/czar.md](crates/czar.md) |
| **czar-macros**    | Proc-macro crate backing `#[task]`                    | [docs/dev/crates/czar-macros.md](crates/czar-macros.md) |
| **czar-tests**     | Cross-crate integration tests (not published)         | [docs/dev/crates/czar-tests.md](crates/czar-tests.md) |

## Dependency Direction

- [czar-core](crates/czar-core.md) is the base layer (traits + models + scheduler).
- State backend crates and [czar-executors](crates/czar-executors.md) implement `czar-core` traits.
- [czar-cli](crates/czar-cli.md) wires everything together and runs the scheduler loop.
- [czar-web](crates/czar-web.md) reads and writes Postgres state via [czar-store](crates/czar-store.md).
- [czar-worker](crates/czar-worker.md) executes isolated worker duties backed by [czar-core](crates/czar-core.md) and [czar-executors](crates/czar-executors.md).
- [czar](crates/czar.md) is consumed by user task code and is intentionally independent from runtime orchestration crates.

## Where to go next

- Want to understand the scheduler loop? Start in [docs/dev/crates/czar-core.md](crates/czar-core.md).
- Want to understand storage? Start in [docs/dev/crates/czar-store.md](crates/czar-store.md).
- Want to add a new executor? Start in [docs/dev/crates/czar-executors.md](crates/czar-executors.md).
