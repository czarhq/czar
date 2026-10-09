# czar-store-sqlite

_Updated at: 2026-05-22_

SQLite state implementation for local single-file deployments.

## Owns

- [`SqliteDatabase`](../../../crates/czar-store-sqlite/src/lib.rs), implementing the `czar-core` repository traits.
- SQLite migrations in [`migrations_sqlite/`](../../../crates/czar-store-sqlite/migrations_sqlite/).
- SQLite backend integration tests in [`tests/`](../../../crates/czar-store-sqlite/tests/).

## Notes

- Depends on SQLx with SQLite features.
- Applications that want the usual facade API can enable the `sqlite` feature on `czar-store`.
