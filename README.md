# Czar

Workflow orchestrator designed for near-zero idle cost.

**Docs:** [czarhq.github.io/czar](https://czarhq.github.io/czar/): guides, how czar works, and live architecture diagrams.

> **Early preview.** Czar is young: APIs, CLI flags, workflow YAML, and configuration may change between releases, and it is not yet recommended for production use. Please report security issues privately; see [SECURITY.md](SECURITY.md).

## Examples Covered

1. [README quickstart example](examples/workflows/readme-quickstart/)

## Goals

1. **Low/no cost at idle**: Pay only for actual compute. Built in Rust for minimal resource usage, with an architecture designed to avoid consumption when idle and to target serverless backends that scale to zero (Cloud Run Jobs today).

2. **Parallelization included**: Orchestration and parallel compute in one. Fan out to serverless workers (Cloud Run Jobs) without managing infrastructure. Scale from one task to thousands.

3. **Orchestration as afterthought**: Write business logic first, add orchestration later. Factory-driven, statically-defined DAGs let you focus on what your code does, not how it's scheduled. No framework lock-in in your task code.

4. **Fully open source**: No company behind this, no "enterprise" edition, no feature gating. Apache-2.0 OR MIT licensed. Community-driven.

## Quick Start

### 1. Create a project

Czar runs Python tasks in your project's environment, managed by [uv](https://docs.astral.sh/uv/):

```bash
uv init my-etl && cd my-etl
uv add czar requests
```

### 2. Define a workflow

Save this as `workflow.yaml`:

<!-- doc-snippet: path=examples/workflows/readme-quickstart/workflow.yaml sha256=161ffaaf5a0fd3bf3a3b1ddb944f4c07060f23d57c102809078c74932f354cd9 lines=1-58 -->
```yaml
# workflow.yaml
name: my_etl

types:
  ExtractResult:
    raw_path: str
    user_count: int
  TransformResult:
    active_path: str
    active_count: int
  LoadResult:
    output_path: str

tasks:
  extract:
    executor: python
    file: tasks.py
    function: extract
    input_type:
      api_url: str
      raw_path: str
    output_type: types.ExtractResult
    inputs:
      api_url:
        const: "file://users.json"
      raw_path:
        const: "/tmp/users_raw.json"
    timeout: 600

  transform:
    executor: python
    file: tasks.py
    function: transform
    depends_on: [extract]
    input_type:
      raw_path: str
      active_path: str
    output_type: types.TransformResult
    inputs:
      raw_path:
        ref: tasks.extract.output.raw_path
      active_path:
        const: "/tmp/users_active.json"

  load:
    executor: python
    file: tasks.py
    function: load
    depends_on: [transform]
    input_type:
      active_path: str
      output_path: str
    output_type: types.LoadResult
    inputs:
      active_path:
        ref: tasks.transform.output.active_path
      output_path:
        const: "/tmp/active_users.json"
```

### 3. Write tasks

Save this as `tasks.py`. Write normal Python functions. The `@task` marker is lightweight and the runtime injects `Context` only when you ask for it:

<!-- doc-snippet: path=examples/workflows/readme-quickstart/tasks.py sha256=89b87627fd0c692727900def89ffb1b3090a86f3608c4fd659941575bb4c1d64 lines=1-66 -->
```python
# tasks.py
import json
from pathlib import Path
from typing import TypedDict

from czar import Context, task
import requests

class User(TypedDict):
    id: int
    status: str

class ExtractResult(TypedDict):
    raw_path: str
    user_count: int

class TransformResult(TypedDict):
    active_path: str
    active_count: int

class LoadResult(TypedDict):
    output_path: str


def load_users(api_url: str) -> list[User]:
    if api_url.startswith("file://"):
        relative_path = api_url.removeprefix("file://")
        source_path = Path(relative_path)
        if not source_path.is_absolute():
            source_path = Path(__file__).parent / source_path
        return json.loads(source_path.read_text(encoding="utf-8"))

    response = requests.get(api_url, timeout=30)
    response.raise_for_status()
    return response.json()


@task
def extract(api_url: str, raw_path: str, ctx: Context) -> ExtractResult:
    ctx.log.info(f"Extracting users from {api_url}")
    users = load_users(api_url)
    raw_file = Path(raw_path)
    raw_file.parent.mkdir(parents=True, exist_ok=True)
    raw_file.write_text(json.dumps(users), encoding="utf-8")
    return {"raw_path": raw_path, "user_count": len(users)}

@task
def transform(raw_path: str, active_path: str, ctx: Context) -> TransformResult:
    ctx.log.info(f"Transforming {raw_path} into {active_path}")
    users: list[User] = json.loads(Path(raw_path).read_text(encoding="utf-8"))
    active_users = [user for user in users if user["status"] == "active"]
    active_file = Path(active_path)
    active_file.parent.mkdir(parents=True, exist_ok=True)
    active_file.write_text(json.dumps(active_users), encoding="utf-8")
    return {"active_path": active_path, "active_count": len(active_users)}

@task
def load(active_path: str, output_path: str, ctx: Context) -> LoadResult:
    ctx.log.info(f"Writing final artifact to {output_path}")
    output_file = Path(output_path)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(
        Path(active_path).read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    return {"output_path": output_path}
```

The workflow reads its users from `users.json`, next to it, so it runs offline:

<!-- doc-snippet: path=examples/workflows/readme-quickstart/users.json sha256=057e1123b9e0525791d1e3f1a669424d947f1838063cf62a5aff8d1a064bd41c lines=1-5 -->
```json
[
  { "id": 1, "status": "active" },
  { "id": 2, "status": "inactive" },
  { "id": 3, "status": "active" }
]
```

The same example is in [examples/workflows/readme-quickstart](examples/workflows/readme-quickstart/). Replace that input with an `https://...` endpoint when you want a real API source.

Czar still calls your functions directly. `@task` is only an authoring marker, and `ctx: Context` is optional metadata injection. Prefer strongly typed, small inputs/outputs (paths, IDs, counts), and avoid passing large payloads between tasks. When you declare `input_type` or `output_type` for Python tasks, `czar validate-workflow` checks those declarations against the function annotations.

### 4. Install and run

```bash
cargo install czar-cli --locked
czar run workflow.yaml
```

State goes to `~/.czar/czar.db` (SQLite) unless `DATABASE_URL` points at Postgres. The latest development version, without the web UI (only published releases embed it): `cargo install --git https://github.com/czarhq/czar czar-cli --locked`.

`czar run` streams task logs by default. Use `--silent` to suppress live logs or `--verbose` to also print task lifecycle transitions.

For contributors installing from a local clone, use `just install` (see [docs/dev/running.md](docs/dev/running.md)).

**First time?** Start the server first to see the UI:

```bash
# terminal 1 - start the scheduler and UI server
czar serve

# terminal 2 - run your workflow
czar run workflow.yaml
```

Then open [http://127.0.0.1:4000](http://127.0.0.1:4000) to see the workflow execution in the UI.

`czar serve` uses the current working directory as its workflow definitions root by default. Pass `--workflows-dir <path>` to point reload/bootstrap at a different project tree explicitly.

### 4b. Install through Python packaging

Python projects can get the task SDK and a prebuilt `czar` command from PyPI:

```bash
uv add czar                # task SDK: `from czar import Context, task`
uv add --dev "czar[cli]"   # adds the czar-cli wheel from PyPI, which ships the `czar` binary
uv run czar run workflow.yaml
```

The `czar-cli` wheel is platform-specific. If there is no wheel for your platform, use `cargo install` as shown above.

### 4c. Start the UI and open it in your browser

`czar run workflow.yaml` executes a workflow, but does not start the web UI.

To run the scheduler + UI locally:

```bash
# terminal 1 - start the scheduler and UI server
czar serve
```

Then open:

<!-- doc-snippet: path=examples/workflows/readme-quickstart/ui-url.txt sha256=7dc2c8e2f2e412e38c355e63a2c6bc01d7f0227ebbad1247d0ecd91062be1bdc lines=1-1 -->
```text
http://127.0.0.1:4000
```

You can keep terminal 1 running, and in terminal 2 run workflows with `czar run workflow.yaml`.

Note: `czar serve` starts the scheduler with the web UI. The SQLite database defaults to `~/.czar/czar.db` (`sqlite://$HOME/.czar/czar.db?mode=rwc`); set `DATABASE_URL` to use another file.

For contributor-focused DB-backed scheduler + web UI setup, see [Running Locally](docs/dev/running.md).

### 5. Deploy

See [Local Development](docs/deployment/local.md) for the current local setup.

## Documentation

All of this is also on the [docs site](https://czarhq.github.io/czar/), with search and live diagrams.

**Getting Started:**
- [Quick Start](docs/guide/quickstart.md) - Get up and running in 5 minutes
- [Examples Guide](docs/guide/examples.md) - Shared prerequisites and run instructions for example workflows
- [Documentation Index](docs/INDEX.md) - Overview of the docs tree

**Writing Workflows:**
- [Concepts](docs/design/concepts.md) - Current workflow, run, flow, and hook model
- [Workflows](docs/design/workflows.md) - DAG authoring model and dependency semantics
- [Tasks](docs/design/tasks.md) - Current Python task authoring and validation surface
- [Context](docs/design/context.md) - Runtime task metadata and task-side helpers
- [Workflow Schema](docs/design/workflow-schema.md) - Current workflow YAML schema and validation rules
- [Type Schemas](docs/design/type_schemas.md) - Type aliases and task contracts
- [Configuration](docs/design/configuration.md) - Current task settings and scheduler tuning
- [Operations](docs/design/operations.md) - Current CLI workflow, run, and validation commands
- [Executors](docs/design/executors.md) - Current built-in executor model
- [Failure Handling](docs/design/failure_handling.md) - Retries, timeouts, cancellation, hooks, and built-in notifications
- [Native Executors](docs/design/native-executors.md) - Process, library, and wasm executor behavior
- [Parallelization](docs/design/parallelization.md) - Current static, dynamic, and Cloud Run parallel execution modes

**Deployment:**
- [Deployment Overview](docs/deployment/overview.md) - Current deployment entry points
- [Local Development](docs/deployment/local.md) - Current local development setup

**Developer Docs:**
- [How czar works](docs/dev/architecture/README.md) - Building blocks, domain model, flows, known issues
- [Running Locally](docs/dev/running.md) - One-command boot + example execution
- [Releasing](docs/dev/releasing.md) - Cargo publish prep and release order
- [Release Checklist](docs/dev/release-checklist.md) - Step-by-step release execution checklist
- [Schema](docs/dev/schema.md) - Database structure, object storage, JSON formats
- [Crates](docs/dev/crates.md) - Rust crate structure and interfaces

## License

Apache-2.0 OR MIT
