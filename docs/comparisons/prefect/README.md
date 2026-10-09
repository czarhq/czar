# Prefect Examples

_Updated at: 2026-05-22_

Prefect is a modern Python workflow orchestration framework, designed as a simpler alternative to Airflow.

## Setup

```bash
uv sync
```

## Usage

```bash
# Run flows (works without server)
just run parallel_tasks
just run data_pipeline
just run conditional_branching

# Run all
just run-all

# Start server (optional - for UI)
just server
# Open http://localhost:4200
```

## Examples

- `flows/parallel_tasks.py` - Parallel execution with fan-in
- `flows/data_pipeline.py` - ETL with data passing
- `flows/conditional_branching.py` - Conditionals and loops

## Documentation

[Prefect Docs](https://docs.prefect.io/)
