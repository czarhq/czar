# Azure Data Factory Examples

_Updated at: 2026-05-22_

Azure's cloud ETL and data integration service.

## Setup

Requires Azure account and Azure CLI:

```bash
az login
```

## Deploying

### Via Azure Portal (Easiest)

1. Go to Azure Data Factory portal
2. Create new Data Factory
3. Open "Author" view
4. Click "+" → Pipeline → Import from pipeline template
5. Paste JSON from files
6. Configure linked services and datasets
7. Publish

### Via Azure CLI

```bash
# Create Data Factory
az datafactory create \
  --resource-group myResourceGroup \
  --factory-name myDataFactory \
  --location eastus

# Create pipeline
az datafactory pipeline create \
  --resource-group myResourceGroup \
  --factory-name myDataFactory \
  --name parallel-tasks-flow \
  --pipeline @parallel_tasks.json
```

## Running

### Via Portal

1. Open Data Factory Studio
2. Go to pipelines
3. Select pipeline
4. Click "Debug" or "Add trigger" → "Trigger now"

### Via CLI

```bash
az datafactory pipeline create-run \
  --resource-group myResourceGroup \
  --factory-name myDataFactory \
  --name parallel-tasks-flow

# Monitor
az datafactory pipeline-run show \
  --resource-group myResourceGroup \
  --factory-name myDataFactory \
  --run-id <run-id>
```

## Examples

- `parallel_tasks.json` - ForEach activity with multiple parallel operations
- `data_pipeline.json` - Sequential Copy/Transform activities
- `conditional_branching.json` - If-Else and ForEach activities

Note: JSON files need configuration of actual data sources, sinks, and compute resources.

## Documentation

[Azure Data Factory Docs](https://docs.microsoft.com/en-us/azure/data-factory/)
