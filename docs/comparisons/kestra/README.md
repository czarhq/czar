# Kestra Examples

_Updated at: 2026-05-22_

Event-driven YAML-first orchestration with modern UI.

## Setup

Kestra requires a running server. Options:

### Docker (Easiest)

```bash
# Start Kestra
docker run --rm -p 8080:8080 kestra/kestra:latest server standalone

# Open http://localhost:8080
```

### Deploy Workflows

```bash
# Via UI: Copy/paste YAML files into Kestra UI editor

# Via CLI:
kestra flow validate parallel_tasks.yaml
kestra flow namespace update
kestra flow deploy parallel_tasks.yaml
```

## Running

1. Open Kestra UI: http://localhost:8080
2. Navigate to "Flows"
3. Select a flow
4. Click "Execute"

## Examples

- `parallel_tasks.yaml`
- `data_pipeline.yaml`
- `conditional_branching.yaml`

## Documentation

[Kestra Docs](https://kestra.io/docs/)
