# czar-store

_Updated at: 2026-05-22_

Facade and shared state helpers for Czar. Concrete database implementations live in separate backend crates so features and dependencies stay explicit.

## Owns

- Re-exports selected backend types behind features:
  - `file` -> [`czar-store-file::FileDatabase`](../../../crates/czar-store-file/src/lib.rs)
  - `sqlite` -> [`czar-store-sqlite::SqliteDatabase`](../../../crates/czar-store-sqlite/src/lib.rs)
  - `postgres` -> [`czar-store-postgres::PostgresDatabase`](../../../crates/czar-store-postgres/src/lib.rs)
  - `d1` -> [`czar-store-d1`](czar-store-d1.md)
- Backend bootstrap helpers used by the CLI and services.
- Object-store traits and local object-store implementation.

## Backend Crates

| Crate | Purpose |
|-------|---------|
| [`czar-store-file`](../../../crates/czar-store-file/) | JSON-file local/dev state backend. |
| [`czar-store-sqlite`](../../../crates/czar-store-sqlite/) | SQLite local state backend and SQLite migrations. |
| [`czar-store-postgres`](../../../crates/czar-store-postgres/) | Postgres production backend and Postgres migrations. |
| [`czar-store-d1`](czar-store-d1.md) | Cloudflare D1 backend and Wrangler test harness. |

## Notes

- Downstream application crates can depend on `czar-store` with specific features for the usual facade API.
- Internal crates can depend directly on backend crates when they need a single concrete backend and want dependencies to be maximally clear.
