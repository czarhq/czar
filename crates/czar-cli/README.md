# czar-cli

The `czar` command: run workflows, serve the scheduler and web UI, inspect runs.

## Install

```bash
cargo install czar-cli --locked
```

State goes to SQLite (`~/.czar/czar.db`) unless `DATABASE_URL` points at Postgres. See the [project README](https://github.com/czarhq/czar#readme) for a quick start.

## CLI Commands

```bash
# Workflow definitions
czar load <workflow.yaml> [--project local] [--region local]
czar list-workflows
czar delete-workflow <workflow-name>

# Execution
czar run <workflow.yaml> [--var NAME=VALUE] [--flow <flow-name>] [<selector> ...]
czar trigger <workflow-name>
czar trigger <workflow-name> [--flow <flow-name>] [<selector> ...]

# Runs & Tasks
czar status [--workflow <workflow-name>]
czar tasks <run-id>
czar logs <run-id> [task-name-or-id]

# Service & database
czar serve [--workflows-dir <path>]
czar init
```

Notes:
- `czar serve` starts the scheduler plus the bundled React UI on the same address when the frontend bundle is available.
- `czar serve` uses the current working directory as the workflow definitions root unless you pass `--workflows-dir <path>` or set `CZAR_WORKFLOW_DEFINITIONS_DIR`.
- `czar run` supports workflow var overrides through repeated `--var NAME=VALUE`.
- `czar run` streams task logs by default. Use `--silent` to suppress live logs or `--verbose` to also print task lifecycle transitions.
- Relative workflow paths such as `file:` and path-like `command:` entries are resolved from the workflow YAML directory, not the shell CWD.
- Python workflow validation checks declared `input_type` and `output_type` against function annotations for file-backed Python tasks.
- When a local run fails, use `czar logs <run-id> [task-name-or-id]` to inspect captured task output and error text.

### Trigger task selection specs

Both `czar run` and `czar trigger` support selective execution against the workflow DAG:

- `task` = exact task
- `+task` = task + all dependencies (upstream)
- `task+` = task + all dependants (downstream)
- `+task+` = full neighborhood
- `2+task` = upstream depth 2
- `task+2` = downstream depth 2
- `2+task+1` = upstream depth 2 and downstream depth 1

Multiple selectors are additive by default:

- `czar trigger my_workflow abc def` = run `abc` and `def`

## Configuration

Environment variables:
- `DATABASE_URL` - Database connection; default `sqlite://$HOME/.czar/czar.db?mode=rwc`, or a `postgres://` URL
- `RUST_LOG` - Log level (info, debug)

See [../../docs/dev/architecture/README.md](../../docs/dev/architecture/README.md) for architecture details.
