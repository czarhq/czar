# Local Development

_Updated at: 2026-10-09_

Running Czar locally needs nothing beyond the `czar` CLI. By default it keeps all state in a single SQLite file, so there is no database or container to set up first.

## Examples Covered

1. [Local deployment example](../../examples/deployments/local/)

## Architecture

![czar on one machine: the clients, czar serve, the database and task processes](diagrams/local.png)

One process, `czar serve`, holds the API and web UI, the Scheduler and the Executors. Tasks run as child processes on the same machine (or inside czar for `library` and `wasm` tasks). The CLI doesn't talk to the server: it opens the database directly.

## Quick start

Start the scheduler and web UI:

```bash
czar serve
```

This serves the API and UI at <http://127.0.0.1:4000> and stores state in `sqlite://$HOME/.czar/czar.db?mode=rwc` by default. In another terminal, run a workflow:

```bash
czar run path/to/workflow.yaml
```

See [operations.md](../design/operations.md) for the full CLI surface — `czar workflow deploy`, `czar trigger`, `czar run start`, scheduling, and run inspection.

## Serving beyond localhost

On `127.0.0.1` the API needs no login. On any other address, pick an `--auth` mode, usually a token:

```bash
CZAR_AUTH_TOKEN=$(openssl rand -hex 32) czar serve --addr 0.0.0.0:4000
```

See [Securing the API](api-auth.md).

## Using Postgres instead of SQLite

If you don't have Postgres running yet, start one with Docker:

```bash
docker run -d --name czar-postgres -p 5432:5432 \
  -e POSTGRES_DB=czar -e POSTGRES_USER=user -e POSTGRES_PASSWORD=pass \
  postgres:16
```

Then point Czar at it with `DATABASE_URL`. The CLI opens the database itself too, so export it in every terminal you run `czar` in:

```bash
export DATABASE_URL=postgres://user:pass@localhost:5432/czar
czar serve
```
