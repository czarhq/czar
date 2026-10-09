# czar-store-postgres

_Updated at: 2026-05-22_

Postgres state implementation for production-style deployments.

## Owns

- [`PostgresDatabase`](../../../crates/czar-store-postgres/src/lib.rs), implementing the `czar-core` repository traits.
- Postgres migrations in [`migrations/`](../../../crates/czar-store-postgres/migrations/).
- Postgres backend integration tests and shared disposable-Postgres test support in [`tests/`](../../../crates/czar-store-postgres/tests/).

## Notes

- Depends on SQLx with Postgres features.
- Applications that want the usual facade API can enable the `postgres` feature on `czar-store`.
