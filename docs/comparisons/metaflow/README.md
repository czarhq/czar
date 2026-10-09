# Metaflow Examples

_Updated at: 2026-05-22_

Netflix's ML/AI workflow framework.

## Setup

```bash
uv sync
```

## Usage

```bash
# Run flows
just run parallel_tasks
just run data_pipeline
just run conditional_branching

# Run all
just run-all

# List recent runs
just list

# Server helper (placeholder)
just server
# Prints setup guidance for Metaflow UI service
```

## Examples

- `flows/parallel_tasks.py`
- `flows/data_pipeline.py`
- `flows/conditional_branching.py`

## Documentation

[Metaflow Docs](https://docs.metaflow.org/)
