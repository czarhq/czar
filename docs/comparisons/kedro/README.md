# Kedro Examples

_Updated at: 2026-05-22_

Kedro is a production-ready data science pipeline framework emphasizing software engineering best practices.

## Setup

```bash
uv sync
```

## Usage

```bash
# Run pipelines
just run parallel_tasks
just run data_pipeline
just run conditional_branching

# Run all
just run-all

# Direct Python execution (simplified standalone)
just direct parallel_tasks
```

Note: These are simplified standalone scripts. Real Kedro requires `kedro new` project structure.

## Examples

- `pipelines/parallel_tasks.py` - Parallel nodes (simplified)
- `pipelines/data_pipeline.py` - ETL pipeline (simplified)
- `pipelines/conditional_branching.py` - Conditional logic (simplified)

## Documentation

[Kedro Docs](https://docs.kedro.org/)
