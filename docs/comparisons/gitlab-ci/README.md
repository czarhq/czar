# GitLab CI Examples

_Updated at: 2026-05-22_

GitLab's integrated CI/CD with advanced DAG pipelines.

## Setup

These workflows run on GitLab's infrastructure. Python scripts are in `scripts/` directory.

## Running on GitLab

1. Push to GitLab repo:
   ```bash
   # Rename one workflow to .gitlab-ci.yml (GitLab only uses this name)
   cp parallel_tasks.yaml .gitlab-ci.yml
   git add .gitlab-ci.yml scripts/
   git commit -m "Add GitLab CI pipeline"
   git push
   ```

2. Go to your GitLab repo → CI/CD → Pipelines
3. Pipeline starts automatically on push, or click "Run pipeline"

## Switching Between Examples

```bash
# Use different example
cp data_pipeline.yaml .gitlab-ci.yml
git add .gitlab-ci.yml && git commit -m "Switch to data pipeline" && git push

# Or use conditional_branching
cp conditional_branching.yaml .gitlab-ci.yml
```

## Local Testing

Test Python scripts locally:

```bash
python scripts/extract.py
python scripts/transform.py
python scripts/check_quality.py
```

## Examples

- `parallel_tasks.yaml` - Parallel stages
- `data_pipeline.yaml` - ETL with artifacts
- `conditional_branching.yaml` - Conditionals with parallel matrix
- `scripts/` - Python scripts called by CI jobs

## Documentation

[GitLab CI Docs](https://docs.gitlab.com/ee/ci/)
