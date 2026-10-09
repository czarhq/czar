# Deferrables Example

This example demonstrates how to use deferrables in Czar workflows to track long-running external jobs.

Shared example setup and prerequisites live in [docs/guide/examples.md](../../../docs/guide/examples.md).

## What are Deferrables?

Deferrables allow tasks to start long-running external jobs (BigQuery queries, Cloud Run jobs, Dataproc jobs, etc.) and return immediately. The Czar scheduler then tracks these jobs via the Triggerer component, allowing worker containers to scale to zero while jobs run.

## Key Benefits

1. **Resource Efficiency**: Worker containers don't sit idle waiting for jobs to complete
2. **Scalability**: Workers can scale to zero when not actively executing tasks
3. **Reliability**: Job tracking survives scheduler restarts (tracked in database)
4. **Flexibility**: Support for multiple external services (BigQuery, Cloud Run, Dataproc, custom HTTP)

## How It Works

```
1. Task starts external job (e.g., BigQuery query)
2. Task returns deferrable with job_id
3. Worker scales to 0
4. Scheduler's Triggerer polls external API for job status
5. Job completes → Triggerer notifies scheduler
6. Task marked as complete → downstream tasks can run
```

## Example Tasks

### 1. BigQuery Job
```python
from czar import BigQueryJob

def run_analytics():
    # Start BigQuery job
    job = bigquery_client.query("SELECT * FROM dataset.table")

    # Return deferrable
    return {"deferred": [BigQueryJob(
        project="my-project",
        job_id=job.job_id,
        location="US"
    ).to_dict()]}
```

### 2. Custom HTTP Job
```python
from czar import CustomHttp

def trigger_external_job():
    job_id = start_external_job()

    return {"deferred": [CustomHttp(
        url=f"https://api.example.com/jobs/{job_id}/status",
        method="GET",
        completion_field="status",
        completion_value="completed"
    ).to_dict()]}
```

### 3. Multiple Deferred Jobs
```python
def run_parallel_jobs():
    # Start multiple jobs
    bq_job = start_bigquery_job()
    cr_job = start_cloudrun_job()

    # Return multiple deferrables
    # Task completes when ALL jobs complete
    return {"deferred": [
        BigQueryJob(...).to_dict(),
        CloudRunJob(...).to_dict()
    ]}
```

## Running the Example

This example uses simulated jobs for demonstration.

```bash
just example deferrables
```

## Workflow Structure

```yaml
tasks:
  bigquery_job:
    executor: python
    file: tasks.py
    # Returns BigQueryJob deferrable

  process_results:
    executor: python
    file: tasks.py
    depends_on: [bigquery_job]  # Runs after bigquery_job completes
```

## Supported Deferrable Types

### Built-in Deferrables

1. **BigQueryJob** - Track BigQuery queries
   - Fields: `project`, `job_id`, `location`
   - Polls: BigQuery Jobs API

2. **CloudRunJob** - Track Cloud Run job executions
   - Fields: `project`, `region`, `job_name`, `execution_id`
   - Polls: Cloud Run Executions API

3. **DataprocJob** - Track Dataproc Spark/Hadoop jobs
   - Fields: `project`, `region`, `cluster_name`, `job_id`
   - Polls: Dataproc Jobs API

4. **CustomHttp** - Track custom API endpoints
   - Fields: `url`, `method`, `headers`, `completion_field`, `completion_value`
   - Polls: Custom HTTP endpoint

## Real-World Use Cases

1. **ETL Pipelines**: BigQuery transformations that take minutes/hours
2. **ML Training**: Dataproc Spark jobs for model training
3. **Batch Processing**: Cloud Run jobs for large-scale data processing
4. **External APIs**: Integration with third-party services via HTTP polling

## Implementation Notes

- Deferrables are tracked in the `deferred_jobs` database table
- Triggerer polls jobs every 10 seconds (configurable)
- Job state survives scheduler restarts
- Downstream tasks wait for all deferred jobs to complete
- Failed jobs propagate errors to task status
