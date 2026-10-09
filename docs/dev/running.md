# Running Czar Locally

_Updated at: 2026-05-22_

These are contributor/developer instructions for the full local stack (Postgres + orchestrator API + React frontend) running in Docker.

## 0) Install the CLI from your local clone

```bash
just install
```

This installs the local `czar` binary from `crates/czar-cli` with `sqlite,local-executors` features so local examples use the same executor set as `just example`.

## 1) Boot services

In terminal 1:

```bash
just up
```

What this does:
- Uses `crates/czar-cli/docker-compose.dev.yml` with dev-specific container definitions
- Runs the orchestrator from a non-release Rust build inside the container
- Starts the React frontend in Vite dev mode inside Docker
- Bootstraps workflow definitions from the checked-out repo via bind mount
- Exposes the frontend at `http://127.0.0.1:5175`
- Exposes the API at `http://127.0.0.1:4000`

Leave this running in one terminal.

## 1a) Non-dev container build

The non-dev container definitions remain in:
- `crates/czar-cli/Dockerfile`
- `crates/czar-cli/docker-compose.yml`
- `crates/czar-web-react/Dockerfile`

Those are intended for packaged/release-style container builds, not day-to-day development.

## 1b) Boot services (SQLite, no Docker)

```bash
just up-sqlite
```

This runs the local scheduler/API against `sqlite://./.czar/czar.db?mode=rwc` without Docker and starts the React frontend in Vite dev mode. Set `DATABASE_URL` to use a different SQLite file (e.g. `sqlite://./tmp/czar.db?mode=rwc`). `czar serve` uses the current working directory as the workflow definitions root by default; pass `--workflows-dir <path>` if you want reload/bootstrap to target a different tree explicitly. Set `CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS=0` if you explicitly want to disable startup reloads. The local executor set in this mode is the `local-executors` feature alias: process, library, and embedded wasm support.

## 2) Open the app

Open `http://127.0.0.1:5175` and trigger one of the bootstrapped example workflows from the UI.

## Optional

- Change the DB connection: `DATABASE_URL=postgres://... just up`
- List all workflows: `cargo run -p czar-cli --bin czar -- list-workflows`
- Run release-prep checks locally: `just publish-prep`
- Run full publish dry-run checks (after bootstrap): `just publish-prep-full`
