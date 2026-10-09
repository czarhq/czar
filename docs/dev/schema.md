# Database Schema

_Updated at: 2026-05-22_

Czar uses a relational database (PostgreSQL or SQLite) for all state management.

## Core Tables

- **workflows** - Workflow definitions and defaults (executor labels, Cloud Run metadata)
- **workflow_tasks** - Compiled DAG tasks for each workflow (executor + dependencies + params)
- **runs** - Workflow execution instances
- **tasks** - Individual task executions within a run (includes `task_name` + `depends_on` for DAGs)

## Schema Source

The complete canonical schema showing the final state after all migrations is in [crates/czar-cli/schema.sql](../../crates/czar-cli/schema.sql). This file is auto-generated from the database using `just dump-schema`.

Each backend keeps its own ordered migration set in its crate; those directories are the source of truth, and the filenames describe what each migration adds:

- Postgres: [crates/czar-store-postgres/migrations/](../../crates/czar-store-postgres/migrations/)
- SQLite: [crates/czar-store-sqlite/migrations_sqlite/](../../crates/czar-store-sqlite/migrations_sqlite/)
- Cloudflare D1: [crates/czar-store-d1/migrations/](../../crates/czar-store-d1/migrations/)

Migrations are applied automatically by `czar init`, `czar serve`, `czar dev`, and the CLI commands that write: `czar run`, `czar workflow`, `czar trigger`, `czar load` and `czar delete-workflow` (Postgres/SQLite via sqlx; D1 via its own runner).

## Models

The Rust models that map to these tables are in [crates/czar-core/src/models.rs](../../crates/czar-core/src/models.rs):

- `Workflow` - Maps to workflows table
- `Run` - Maps to runs table
- `Task` - Maps to tasks table
- `TaskWithWorkflow` - JOIN result for optimized queries
- `WorkflowTask` - Compiled DAG rows stored per workflow

## Database Implementations

Four database backends implement the `Database` trait, each in its own crate (see [crates/czar-store.md](crates/czar-store.md)):

### PostgresDatabase

Implementation: [crates/czar-store-postgres/src/lib.rs](../../crates/czar-store-postgres/src/lib.rs)

- Uses sqlx connection pool
- Batch operations with transactions
- Composite indexes for performance; this is the production-oriented backend

### SqliteDatabase

Implementation: [crates/czar-store-sqlite/src/lib.rs](../../crates/czar-store-sqlite/src/lib.rs)

- Single-file SQLite via sqlx; the default local backend (`sqlite://$HOME/.czar/czar.db?mode=rwc`)

### D1Database

Implementation: [crates/czar-store-d1/src/lib.rs](../../crates/czar-store-d1/src/lib.rs)

- Cloudflare D1 over its HTTP API

### FileDatabase

Implementation: [crates/czar-store-file/src/lib.rs](../../crates/czar-store-file/src/lib.rs)

- JSON file storage: `{base}/workflows/`, `{base}/runs/`, `{base}/tasks/`
- Compiled DAGs stored in `{base}/workflow_tasks/`
- Uses serde_json for serialization
- No indexes (file-based, not query-optimized)
- Good for development/testing

## Query Patterns

The key performance optimization is using JOIN queries to avoid N+1 problems. See `get_pending_tasks_with_workflow()` in the Database trait implementations for examples.
