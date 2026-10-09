# Local Deployment

Run Czar locally with nothing but the `czar` CLI. State lives in a single SQLite file (`~/.czar/czar.db`) by default, and the scheduler, web UI, and executors all run in one process; see the diagram in [docs/deployment/local.md](../../../docs/deployment/local.md).

The full walkthrough is [docs/deployment/local.md](../../../docs/deployment/local.md).

## Usage

Start the scheduler and web UI (served at <http://127.0.0.1:4000>):

```bash
czar serve
```

In another terminal, deploy a workflow and start a run:

```bash
czar workflow deploy path/to/workflow.yaml
czar run start <workflow-name>
```

`czar run start` prints the run ID. Follow its logs with:

```bash
czar run logs <run-id> --follow
```
