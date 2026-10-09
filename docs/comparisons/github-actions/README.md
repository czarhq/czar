# GitHub Actions Examples

_Updated at: 2026-05-22_

GitHub's integrated CI/CD and workflow orchestration.

## Setup

These workflows run on GitHub's infrastructure - no local setup needed for actual execution.

For local development, Python scripts are in `scripts/` directory.

## Running on GitHub

1. Push workflows to `.github/workflows/` in your repo:
   ```bash
   mkdir -p .github/workflows
   cp *.yaml .github/workflows/
   cp -r scripts/ .github/scripts/
   git add .github/
   git commit -m "Add workflows"
   git push
   ```

2. Go to your GitHub repo → Actions tab
3. Select a workflow
4. Click "Run workflow"

## Local Testing

Test Python scripts locally:

```bash
# Test individual scripts
python scripts/fetch_users.py
python scripts/extract.py
python scripts/check_quality.py
```

## Examples

- `parallel_tasks.yaml` - Parallel jobs with fan-in
- `data_pipeline.yaml` - ETL with data passing via artifacts
- `conditional_branching.yaml` - Conditional execution with matrix strategy
- `scripts/` - Python scripts called by workflows

## Documentation

[GitHub Actions Docs](https://docs.github.com/en/actions)
