# Czar SDK for Python

Python SDK for writing Czar workflow tasks with support for deferrables.

## Installation

For task authoring and lightweight examples:

```bash
uv add czar
```

For local CLI tooling through Python packaging:

```bash
uv add "czar[cli]"
```

## Usage

### Basic Task

```python
def my_task(input_data):
    result = process_data(input_data)
    return result
```

### Deferrable Task (Long-Running Jobs)

Deferrables allow tasks to start long-running external jobs and return immediately. The Czar scheduler tracks these jobs, allowing workers to scale to zero.

```python
from czar import BigQueryJob
from google.cloud import bigquery

def analyze_data():
    client = bigquery.Client(project="my-project")
    job = client.query("""
        SELECT * FROM dataset.table
        WHERE date >= '2024-01-01'
    """)

    # Return deferrable - worker can scale to 0
    return BigQueryJob(
        project="my-project",
        job_id=job.job_id,
        location="US"
    )
```

## Supported Deferrables

### BigQueryJob

Track BigQuery queries:

```python
from czar import BigQueryJob

return BigQueryJob(
    project="my-project",
    job_id=job.job_id,
    location="US"
)
```

### CloudRunJob

Track Cloud Run job executions:

```python
from czar import CloudRunJob

return CloudRunJob(
    project="my-project",
    region="us-central1",
    job_name="my-job",
    execution_id=execution_id
)
```

### DataprocJob

Track Dataproc Spark/Hadoop jobs:

```python
from czar import DataprocJob

return DataprocJob(
    project="my-project",
    region="us-central1",
    cluster_name="my-cluster",
    job_id=job_id
)
```

### CustomHttp

Track custom external services via HTTP polling:

```python
from czar import CustomHttp

return CustomHttp(
    url=f"https://api.example.com/jobs/{job_id}/status",
    method="GET",
    headers={"Authorization": "Bearer token"},
    completion_field="status",
    completion_value="completed",
    failure_value="failed"
)
```

## Multiple Deferrables

Tasks can return multiple deferrables to track multiple jobs:

```python
from czar import BigQueryJob, CloudRunJob

def complex_job():
    # Start multiple jobs
    bq_job = start_bigquery_job()
    cr_job = start_cloudrun_job()

    # Return both - task completes when ALL complete
    return [
        BigQueryJob(project="p1", job_id=bq_job.job_id, location="US"),
        CloudRunJob(project="p1", region="us-central1", job_name="job", execution_id=cr_job.id)
    ]
```

## License

Apache-2.0 OR MIT
