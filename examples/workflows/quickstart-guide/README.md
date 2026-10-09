# Quickstart Guide Example

Source files referenced by `docs/guide/quickstart.md`.

Run the source script directly:

```bash
uv run python run.py
```

Inspect the inferred task schema:

```bash
cargo run -p czar-cli -- schema export tasks_fetch.py
```

Validate and execute the workflow locally:

```bash
cargo run -p czar-cli -- validate-workflow workflow.yaml
cargo run -p czar-cli -- run workflow.yaml
```
