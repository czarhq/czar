# Workflow Orchestration Framework Comparisons

_Updated at: 2026-05-22_

This directory contains working examples of various orchestration frameworks, with each framework implementing the same three examples for easy comparison.

## Comparison

See **[comparison.md](comparison.md)** for a feature-by-feature comparison of all frameworks across workflow definition, runtime control, integration, and tooling/UX.

## Examples

All frameworks implement these three benchmark examples:

1. **[examples/parallel_tasks.md](examples/parallel_tasks.md)** - Parallel execution with fan-in dependency
2. **[examples/data_pipeline.md](examples/data_pipeline.md)** - Simple ETL with data passing
3. **[examples/conditional_branching.md](examples/conditional_branching.md)** - Conditionals, branching, and loops

Each `.md` file defines the exact scenario, tasks, and expected behavior that all frameworks implement.

## Directory Structure

Each framework has its own directory with the three examples:

```
docs/comparisons/
├── comparison.md               # Feature-by-feature comparison table
├── examples/
│   ├── parallel_tasks.md       # Example 1 definition
│   ├── data_pipeline.md        # Example 2 definition
│   └── conditional_branching.md # Example 3 definition
├── airflow/
│   ├── parallel_tasks.py
│   ├── data_pipeline.py
│   ├── conditional_branching.py
│   ├── pyproject.toml
│   ├── pyrightconfig.json
│   └── ruff.toml
├── prefect/                    # Same structure
├── dagster/
├── luigi/
├── mage/
├── kedro/
├── metaflow/
├── kestra/                     # YAML files
├── github-actions/             # YAML workflows
├── gitlab-ci/
├── aws-step-functions/         # JSON files
├── azure-data-factory/
├── databricks-workflows/
├── czar/                        # Current Czar + proposed designs
└── ...
```

## Available Frameworks

### General Purpose Orchestration
- **Czar** - YAML workflows with Python (or process/container/wasm/library) tasks
- **Airflow** - Apache Airflow with @task/@dag decorators (Python + uv)
- **Prefect** - Prefect with @task/@flow decorators (Python + uv)
- **Dagster** - Dagster with @asset decorators (Python + uv)
- **Luigi** - Spotify's Luigi with Task classes (Python + uv)
- **Mage** - Mage with specialized decorators (Python + uv)

### MLOps / Data Science (Python + uv)
- **Kedro** - Production data science pipelines
- **Metaflow** - Netflix's FlowSpec-based orchestration

### YAML-Based
- **Kestra** - YAML workflows with inline scripts

### CI/CD
- **GitHub Actions** - GitHub workflow YAML (using Python)
- **GitLab CI** - GitLab pipeline YAML

### Cloud-Managed (JSON / YAML)
- **AWS Step Functions** - AWS state machine definitions
- **Azure Data Factory** - Azure pipeline definitions
- **Databricks Workflows** - Databricks job configurations
- **GCP Workflows** - Google Cloud Workflows YAML definitions

## Justfile-Managed Frameworks

The root comparisons `justfile` currently manages these runnable framework folders:

- `airflow`
- `prefect`
- `dagster`
- `luigi`
- `mage`
- `kedro`
- `metaflow`
- `kestra`

## Running Python Examples

Each Python framework directory is a standalone uv project:

```bash
cd airflow/
uv run parallel_tasks.py
uv run data_pipeline.py
uv run conditional_branching.py
```

## Linting and Type Checking

Use the provided `justfile` for consistent linting and type checking across all projects:

```bash
# Check all projects (lint + typecheck)
just check-all

# Run linting across Python projects
just lint-all

# Run type checking across Python projects
just typecheck-all

# Auto-fix linting issues in all projects
just fix-all

# Run a specific example
just run airflow parallel_tasks

# Start framework UI/server
just server airflow

# List justfile-managed frameworks
just list

# Sync pyright and ruff configs from airflow/ to all projects
just sync-configs

# Install dependencies for all projects
just install-all

# Clean all virtual environments
just clean-all
```

All Python projects use:
- `uv` for dependency management (not pip)
- `pyright` with strict type checking (see pyrightconfig.json)
- `ruff` for linting with comprehensive rules (see ruff.toml)

## Type Checking Configuration

All Python projects share the same linting and type checking configuration.

**Shared configs** (edit `airflow/pyrightconfig.json` and `airflow/ruff.toml`, then run `just sync-configs`):

**pyrightconfig.json**:
```json
{
  "typeCheckingMode": "strict",
  "pythonVersion": "3.12",
  "venvPath": ".",
  "venv": ".venv",
  "reportMissingImports": true,
  "reportMissingTypeStubs": false,
  "reportUnknownMemberType": false,
  "reportUnknownVariableType": false,
  "reportUnknownArgumentType": false,
  "reportUnknownParameterType": false,
  "reportUnknownLambdaType": false,
  "reportUntypedFunctionDecorator": false,
  "reportUntypedClassDecorator": false,
  "reportUntypedBaseClass": false,
  "reportIncompatibleMethodOverride": false
}
```

**ruff.toml**:
```toml
[lint]
select = ["E", "F", "I", "UP", "B", "C4", "SIM", "RUF"]
```

These settings balance strictness with practicality for working with orchestration libraries that have incomplete type stubs.

## Comparison Approach

Compare the same example across different frameworks to understand:
- Syntax and ergonomics
- How dependencies are declared
- How data is passed between tasks
- How conditionals and loops are handled
- Verbosity and complexity trade-offs

For example, to compare how different frameworks handle data passing:
```bash
cat airflow/data_pipeline.py
cat prefect/data_pipeline.py
cat dagster/data_pipeline.py
```
