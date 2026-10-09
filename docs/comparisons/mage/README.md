# Mage Examples

_Updated at: 2026-05-22_

Mage is a modern data engineering and MLOps platform with notebook-style pipelines and a user-friendly UI.

## Setup

```bash
uv sync
```

## Usage

```bash
# Initialize (first time only)
just init

# Start server (required)
just server
# Open http://localhost:6789

# Import pipeline files via UI
```

## Examples

- `pipelines/parallel_tasks.py` - Parallel execution with custom blocks
- `pipelines/data_pipeline.py` - ETL with data_loader, transformer, data_exporter
- `pipelines/conditional_branching.py` - Conditional logic with dynamic blocks

## Documentation

[Mage Docs](https://docs.mage.ai/)
