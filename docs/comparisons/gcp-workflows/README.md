# GCP Workflows Examples

_Updated at: 2026-05-22_

These examples demonstrate how GCP Workflows orchestrates Python code running in Cloud Run.

## Architecture

GCP Workflows is a YAML-based serverless orchestration service that coordinates services via HTTP:
- **Cloud Run** - Containerized Python FastAPI service with multiple endpoints
- **Workflows** - YAML definitions that orchestrate HTTP calls with parallelism, conditionals, and loops

Unlike Python-native frameworks (Airflow/Prefect/Dagster) where orchestration logic lives in Python, GCP Workflows separates concerns:
- Python service contains business logic only (FastAPI endpoints)
- YAML workflows handle orchestration (dependencies, parallelism, branching)

## Why One Service vs Many Functions?

**This approach uses a single Cloud Run service with multiple endpoints** - this is a realistic pattern:
- ✅ One deployment to manage
- ✅ No cold start overhead between steps
- ✅ Easier local development and testing
- ✅ Lower cost (fewer service instances)

**Alternative (not shown):** Separate Cloud Functions for each task
- ❌ 13 separate deployments
- ❌ Cold start latency on each function
- ❌ More expensive (charged per invocation)
- ❌ Only makes sense for truly independent services

## Setup

### 1. Build and Deploy Cloud Run Service

```bash
cd service

# Build container
gcloud builds submit --tag gcr.io/PROJECT_ID/workflow-service

# Deploy to Cloud Run
gcloud run deploy workflow-service \
  --image gcr.io/PROJECT_ID/workflow-service \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated

# Get the service URL
gcloud run services describe workflow-service \
  --platform managed \
  --region us-central1 \
  --format 'value(status.url)'
```

### 2. Update Workflow YAML Files

Replace `PROJECT_ID` in the workflow YAML files with your Cloud Run service URL:

```bash
# Get your Cloud Run URL
SERVICE_URL=$(gcloud run services describe workflow-service \
  --platform managed \
  --region us-central1 \
  --format 'value(status.url)')

# Update workflow files
sed -i "s|https://workflow-service-PROJECT_ID.run.app|${SERVICE_URL}|g" *.yaml
```

### 3. Deploy Workflows

```bash
# Deploy parallel_tasks workflow
gcloud workflows deploy parallel-tasks-flow \
  --source=parallel_tasks.yaml \
  --location=us-central1

# Deploy data_pipeline workflow
gcloud workflows deploy data-pipeline-flow \
  --source=data_pipeline.yaml \
  --location=us-central1

# Deploy conditional_branching workflow
gcloud workflows deploy conditional-branching-flow \
  --source=conditional_branching.yaml \
  --location=us-central1
```

## Running Workflows

### Via gcloud CLI

```bash
# Execute a workflow
gcloud workflows execute parallel-tasks-flow \
  --location=us-central1

# View execution logs
gcloud workflows executions describe-last parallel-tasks-flow \
  --location=us-central1
```

### Via Cloud Console

1. Navigate to [Workflows Console](https://console.cloud.google.com/workflows)
2. Select your workflow
3. Click "Execute"
4. View execution logs and results

## Local Development

Test the Python service locally before deploying:

```bash
cd service

# Install dependencies
pip install -r requirements.txt

# Run locally
uvicorn main:app --reload --port 8080

# Test endpoints
curl -X POST http://localhost:8080/extract
curl -X POST http://localhost:8080/transform \
  -H "Content-Type: application/json" \
  -d '{"records": [1, 2, 3], "source": "api"}'
```

## Key Features Demonstrated

### Parallel Execution
```yaml
parallel:
  branches:
    - fetch_users_branch: ...
    - fetch_orders_branch: ...
    - fetch_products_branch: ...
```
Three HTTP calls execute simultaneously, workflow waits for all to complete.

### Data Passing
```yaml
- extract:
    result: extracted_data
- transform:
    body: ${extracted_data.body}
```
Output from one step becomes input to the next.

### Conditional Branching
```yaml
switch:
  - condition: ${quality_result.body.score > 0.8}
    next: process_high_quality
  - condition: true
    next: clean_data
```
Dynamic routing based on runtime values.

### Loops (Parallel For)
```yaml
parallel:
  for:
    value: partition
    in: ["partition_1", "partition_2", "partition_3"]
    steps:
      - process_partition: ...
```
Process multiple items in parallel with a loop.

## Differences from Python-Native Frameworks

| Aspect | GCP Workflows | Airflow/Prefect/Dagster |
|--------|---------------|-------------------------|
| **Orchestration** | YAML | Python decorators |
| **Execution** | HTTP calls | Direct Python function calls |
| **Language** | Any (HTTP endpoints) | Python-specific |
| **Infrastructure** | Serverless (managed) | Self-hosted or managed |
| **Cold starts** | Possible (Cloud Run) | No (always-running workers) |
| **Cost model** | Pay per execution | Pay for compute time |
| **Local testing** | Service can run locally, workflow requires GCP | Fully local |

## When to Use GCP Workflows

**Good fit:**
- Already using GCP services
- Need to orchestrate microservices
- Want minimal infrastructure management
- Workflows call existing APIs/services
- Language-agnostic orchestration

**Not ideal:**
- Pure Python workflows (use Prefect/Airflow)
- Complex Python logic in orchestration
- Need local/offline execution
- Avoiding vendor lock-in

## Cost Model

**Workflows:** $0.01 per 1,000 internal steps (5,000 free/month) **Cloud Run:** Pay per request + compute time (includes free tier)

Example cost for 1,000 workflow executions/month:
- Workflows: ~$0.10 (well under free tier)
- Cloud Run: ~$1-5 depending on execution time

Much cheaper than maintaining orchestration infrastructure.

## Real-World Use Cases

GCP Workflows is typically used to orchestrate:
1. **Data pipelines** - BigQuery → Dataflow → Cloud Storage
2. **Microservices** - Chain multiple service calls with error handling
3. **ML pipelines** - Vertex AI training → evaluation → deployment
4. **ETL jobs** - Coordinate existing data processing services
5. **Business processes** - Multi-step approvals, notifications, integrations

For pure Python data orchestration, Airflow/Prefect/Dagster are more natural choices.
