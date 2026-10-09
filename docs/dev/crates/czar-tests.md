# czar-tests

_Updated at: 2026-09-29_

Test-only workspace crate (`publish = false`) for integration tests that span several Czar crates.

## Owns

- [`tests/core/`](../../../crates/czar-tests/tests/core/): scheduler, triggerer, schedule processor, DAG execution, conditional tasks, deferrables, pause/resume, retry fallback, and type extraction tests.

## Notes

- Tests run against `czar-core`, `czar-store` (SQLite and Postgres), and the `process` executor from `czar-executors`.
- Each test file is registered as its own `[[test]]` target in `Cargo.toml`.
