# Documentation Index

_Updated at: 2026-10-02_

Complete documentation hierarchy for Czar. Start with the [main README](../README.md) for the project overview and quick start.

## 📚 Guides & Tutorials

- **[guide/quickstart.md](guide/quickstart.md)** - Getting started guide
- **[guide/examples.md](guide/examples.md)** - Shared prerequisites and run instructions for example workflows
- **[guide/tutorial_elt_pipeline.md](guide/tutorial_elt_pipeline.md)** - End-to-end ELT pipeline tutorial

## 🧩 Design & Authoring Surface

The current implemented workflow-authoring and runtime model.

- **[design/concepts.md](design/concepts.md)** - Workflow, run, task, flow, and hook model
- **[design/workflows.md](design/workflows.md)** - Workflow authoring model and DAG semantics
- **[design/workflow-schema.md](design/workflow-schema.md)** - Strict workflow YAML schema and validator rules
- **[design/tasks.md](design/tasks.md)** - Python task authoring surface and validation behavior
- **[design/context.md](design/context.md)** - Python task context and runtime metadata
- **[design/type_schemas.md](design/type_schemas.md)** - Type alias and task contract model
- **[design/configuration.md](design/configuration.md)** - Task and runtime configuration surface
- **[design/operations.md](design/operations.md)** - CLI workflow, run, scheduling, and validation operations
- **[design/executors.md](design/executors.md)** - Built-in executor model
- **[design/native-executors.md](design/native-executors.md)** - Process, library, and wasm executor behavior
- **[design/parallelization.md](design/parallelization.md)** - Static, runtime foreach, and Cloud Run parallel execution
- **[design/failure_handling.md](design/failure_handling.md)** - Retry, cancellation, timeout, hook, and notification behavior
- **[design/dbt.md](design/dbt.md)** - dbt import and first-class node integration

## 🚀 Deployment

- **[deployment/overview.md](deployment/overview.md)** - Deployment entry points and guide map
- **[deployment/local.md](deployment/local.md)** - Local development setup
- **[deployment/cloudflare.md](deployment/cloudflare.md)** - Cloudflare: a Worker, a container running czar serve, and D1

## 🔧 Developer Documentation

### Architecture & Internals
How czar works, in reading order:

- **[dev/architecture/README.md](dev/architecture/README.md)** - How czar works: the big picture
- **[dev/architecture/building-blocks.md](dev/architecture/building-blocks.md)** - The building blocks, processes, routes and deployments
- **[dev/architecture/domain-model.md](dev/architecture/domain-model.md)** - The entities and their lifecycles
- **[dev/architecture/concepts.md](dev/architecture/concepts.md)** - Rules that hold across every flow
- **[dev/architecture/flows/README.md](dev/architecture/flows/README.md)** - The flows, starting with the [life of a run](dev/architecture/flows/life-of-a-run.md)
  - [define](dev/architecture/flows/define.md), [start](dev/architecture/flows/start.md), [schedule](dev/architecture/flows/schedule.md), [execute](dev/architecture/flows/execute.md) ([local](dev/architecture/flows/execute-local.md), [worker](dev/architecture/flows/execute-worker.md), [Cloud Run](dev/architecture/flows/execute-cloud-run.md)), [deferred](dev/architecture/flows/deferred.md), [failures](dev/architecture/flows/failures.md), [control](dev/architecture/flows/control.md), [observe](dev/architecture/flows/observe.md), [access](dev/architecture/flows/access.md)
- **[dev/architecture/known-issues.md](dev/architecture/known-issues.md)** - Where the code doesn't yet do what the pages describe
- **[dev/architecture/writing.md](dev/architecture/writing.md)** - How to write and change these pages

Reference:

- **[dev/architecture/storage.md](dev/architecture/storage.md)** - Store backends, tables, and which block writes each entity
- **[dev/architecture/task-sdks.md](dev/architecture/task-sdks.md)** - The task protocol and the Rust and Python SDKs
- **[dev/schema.md](dev/schema.md)** - Database schema, storage formats
- **[dev/crates.md](dev/crates.md)** - Rust crate structure overview
- **[dev/crates/](dev/crates/)** - Per-crate detail docs (core, cli, executors, web, worker, state backends, …)

### Workers & Remote Execution
- **[dev/czar-worker.md](dev/czar-worker.md)** - Stateless worker server
- **[dev/czar-worker-api.md](dev/czar-worker-api.md)** - Worker HTTP API contract
- **[dev/remote-workers.md](dev/remote-workers.md)** - Remote worker execution path

### Running, Testing & Quality
- **[dev/running.md](dev/running.md)** - Local developer runbook for scheduler + web UI
- **[dev/smoke-tests.md](dev/smoke-tests.md)** - End-to-end smoke suite for CLI, web, and TUI
- **[dev/performance.md](dev/performance.md)** - Performance testing infrastructure overview
- **[dev/performance-orchestrator.md](dev/performance-orchestrator.md)** - Orchestrator benchmark guide
- **[dev/performance-frontend.md](dev/performance-frontend.md)** - Frontend benchmark guide
- **[dev/postgres-fault-injection.md](dev/postgres-fault-injection.md)** - Postgres fault-injection test guide
- **[dev/quality-report.md](dev/quality-report.md)** - Timestamped quality report bundle generation
- **[dev/supply-chain.md](dev/supply-chain.md)** - Dependency freshness, license, and vulnerability policy

### Releasing
- **[dev/releasing.md](dev/releasing.md)** - Cargo publish prep and crate release order
- **[dev/release-checklist.md](dev/release-checklist.md)** - Step-by-step release execution checklist
- **[dev/pypi-releasing.md](dev/pypi-releasing.md)** - PyPI packaging and release steps

## 🔍 Comparisons

- **[comparisons/](comparisons/)** - Feature comparison with 15 workflow orchestrators (Airflow, Prefect, Dagster, Metaflow, Kedro, Luigi, and more)

## 📋 Reports

- **[reports/perf/results.md](reports/perf/results.md)** - Recorded orchestrator benchmark results

---

## Navigation Tips

**If you're new to Czar:**
1. Start with the [main README](../README.md)
2. Read [guide/quickstart.md](guide/quickstart.md)
3. Skim [design/concepts.md](design/concepts.md) for the core model

**If you're contributing:**
1. Read [How czar works](dev/architecture/README.md)
2. Follow the flow you are changing in [dev/architecture/flows/](dev/architecture/flows/README.md)
3. Check [dev/crates.md](dev/crates.md) for code organization
4. Use [dev/running.md](dev/running.md) and [dev/smoke-tests.md](dev/smoke-tests.md) to validate changes

**If you're evaluating Czar:**
1. Read [comparisons/](comparisons/) for feature comparisons
2. Check [How czar works](dev/architecture/README.md) for the system design
3. Review the design and deployment docs for the current implemented surface
