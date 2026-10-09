# Airflow Examples

_Updated at: 2026-05-22_

Apache Airflow is the most widely-used workflow orchestration platform, using Python DAGs (Directed Acyclic Graphs) with decorators.

## Setup

```bash
uv sync
```

## Usage

```bash
# Start Airflow server
just server
# Open http://localhost:8080

# Test DAGs
just test parallel_tasks_flow
just test data_pipeline_flow
just test conditional_branching_flow

# Run all
just test-all

# Direct Python execution (not orchestrated)
uv run dags/parallel_tasks.py
```

## Examples

- `dags/parallel_tasks.py` - Parallel execution with fan-in
- `dags/data_pipeline.py` - ETL with data passing
- `dags/conditional_branching.py` - Conditionals and dynamic mapping

## Documentation

[Airflow Docs](https://airflow.apache.org/docs/)
