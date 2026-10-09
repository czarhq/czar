# Releasing Czar to PyPI

_Updated at: 2026-05-22_

This guide covers the Python packages published for Python users.

## Package Shape

- PyPI distribution: `czar`
- Python import: `czar`
- Source directory: `python-sdk/czar`
- Package metadata: `python-sdk/pyproject.toml`
- Optional CLI distribution: `czar-cli`
- Optional CLI package: `python-cli/` (built with maturin from `crates/czar-cli`)

The base `czar` package is the lightweight task-authoring SDK: decorators, runtime `Context`, progress reporting, dbt helpers, and deferrable job payload types.

The `czar[cli]` extra depends on `czar-cli`, which provides the native `czar` CLI command through Python packaging so Python users can develop with `uv` and `pyproject.toml` without installing Cargo.

## Local Preflight

Run these from the repo root:

```bash
uv build python-sdk
uvx twine check python-sdk/dist/*
uv run --with python-sdk/dist/czar-*.whl python -c "from czar import Context, task, BigQueryJob, czar_progress; print(Context().to_dict())"
```

For the optional CLI binary package:

```bash
./scripts/build-python-cli-wheel.sh
uvx twine check python-cli/dist/*.whl   # this machine's wheel only
```

This builds a wheel for this machine only; the release workflow builds the rest.

## Publish

Push a version tag to `czarhq/czar`; `.github/workflows/release.yml` builds the `czar-cli` wheel for Linux (x86_64, aarch64), macOS (Intel, Apple silicon) and Windows, builds the `czar` SDK, and publishes both to PyPI with trusted publishing (no token in the repository):

```bash
git tag v0.0.1 && git push origin v0.0.1
```

Set up trusted publishing once on PyPI, for each of `czar` and `czar-cli`: add a GitHub publisher with owner `czarhq`, repository `czar`, workflow `release.yml`, environment `pypi`.

