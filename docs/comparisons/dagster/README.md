# Dagster Examples

_Updated at: 2026-05-22_

Dagster is an asset-based orchestration framework focused on data lineage and observability.

## Setup

```bash
uv sync
```

## Usage

```bash
# Run jobs
just run parallel_tasks
just run data_pipeline
just run conditional_branching

# Run all
just run-all

# Start UI server
just server
# Open http://localhost:3000
```

## Examples

- `jobs/parallel_tasks.py` - Parallel execution (uses luigi pattern)
- `jobs/data_pipeline.py` - ETL with data passing (uses luigi pattern)
- `jobs/conditional_branching.py` - Conditionals and dynamic mapping

## Documentation

[Dagster Docs](https://docs.dagster.io/)
