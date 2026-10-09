# Databricks Workflows Examples

_Updated at: 2026-05-22_

Databricks Workflows (formerly Databricks Jobs) allows you to orchestrate data pipelines on the Databricks platform. Workflows are defined as JSON job configurations.

## What is Databricks Workflows?

Databricks Workflows is a managed orchestration service that runs on the Databricks platform. It's designed for data engineering and ML pipelines that need to run notebooks, Python scripts, JARs, or SQL queries.

## How to Use These Examples

### Option 1: Via Databricks UI

1. Go to your Databricks workspace
2. Navigate to **Workflows** (or **Jobs** in older versions)
3. Click **Create Job**
4. Switch to **JSON** mode
5. Paste the contents of any `.json` file
6. Click **Create**

### Option 2: Via Databricks CLI

Install the Databricks CLI:
```bash
pip install databricks-cli
```

Configure authentication:
```bash
databricks configure --token
# Enter your workspace URL and access token
```

Create a job from JSON:
```bash
databricks jobs create --json-file parallel_tasks.json
```

Run the job:
```bash
databricks jobs run-now --job-id <job-id>
```

### Option 3: Via REST API

```bash
curl -X POST https://<databricks-instance>/api/2.1/jobs/create \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d @parallel_tasks.json
```

## File Structure

Each JSON file defines a complete Databricks job with:

- **tasks**: Array of task definitions
- **task dependencies**: Using `depends_on` field
- **task types**:
  - `notebook_task` - Run a Databricks notebook
  - `python_wheel_task` - Run Python code from a wheel
  - `spark_python_task` - Run PySpark script
  - `condition_task` - Conditional branching (tier 3)
  - `for_each_task` - Dynamic task mapping (tier 3)

## Understanding the JSON Structure

```json
{
  "name": "parallel_tasks",
  "tasks": [
    {
      "task_key": "fetch_users",           // Unique task identifier
      "notebook_task": {                    // Task type
        "notebook_path": "/path/to/notebook",
        "source": "WORKSPACE"
      },
      "new_cluster": {                      // Compute resources
        "spark_version": "13.3.x-scala2.12",
        "node_type_id": "i3.xlarge",
        "num_workers": 2
      }
    },
    {
      "task_key": "combine_data",
      "depends_on": [                       // Dependencies
        {"task_key": "fetch_users"},
        {"task_key": "fetch_orders"},
        {"task_key": "fetch_products"}
      ],
      "notebook_task": { /* ... */ }
    }
  ]
}
```

## Examples Explained

### parallel_tasks.json
Three tasks run in parallel, then a fourth task waits for all three to complete.
- No dependencies = parallel execution
- `depends_on` array = fan-in pattern

### data_pipeline.json
Linear ETL pipeline: extract → transform → load
- Uses task values to pass data: `{{tasks.extract.values.output_data}}`
- Each task depends on the previous one

### conditional_branching.json
Complex workflow with conditionals and loops:
- `condition_task` for branching logic
- `for_each_task` for dynamic parallel execution
- `depends_on` with `outcome: "true"/"false"` to follow branches

## Notebooks vs Python Tasks

The examples show `notebook_task` for simplicity, but you can replace with:

**Python wheel task**:
```json
"python_wheel_task": {
  "package_name": "my_package",
  "entry_point": "main",
  "parameters": ["arg1", "arg2"]
}
```

**Spark Python task**:
```json
"spark_python_task": {
  "python_file": "s3://bucket/script.py",
  "parameters": ["arg1", "arg2"]
}
```

## Key Differences from Other Frameworks

- **No local execution**: Runs only on Databricks cloud
- **Managed compute**: Databricks manages cluster lifecycle
- **Built for data/ML**: Optimized for Spark, Delta Lake, MLflow
- **Task values**: Custom pattern for passing data between tasks
- **JSON-only**: No Python/YAML DSL (unlike Airflow/Prefect)

## Documentation

- [Databricks Jobs API](https://docs.databricks.com/api/workspace/jobs)
- [Workflows Documentation](https://docs.databricks.com/workflows/)
- [Task values for data passing](https://docs.databricks.com/workflows/jobs/tasks.html#task-values)
