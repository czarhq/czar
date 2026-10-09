# Luigi Examples

_Updated at: 2026-05-22_

Luigi is Spotify's batch job framework for building complex pipelines with dependency management and visualization.

## Setup

```bash
uv sync
```

## Usage

```bash
# Run tasks
just run parallel_tasks CombineData
just run data_pipeline Load
just run conditional_branching CombineResults

# Run all
just run-all

# Start scheduler UI (optional)
just server
# Open http://localhost:8082

# Clean output files
just clean
```

## Examples

- `tasks/parallel_tasks.py` - Parallel execution with file targets
- `tasks/data_pipeline.py` - ETL pipeline with JSON file passing
- `tasks/conditional_branching.py` - Conditional execution with dynamic tasks

## Documentation

[Luigi Docs](https://luigi.readthedocs.io/)
