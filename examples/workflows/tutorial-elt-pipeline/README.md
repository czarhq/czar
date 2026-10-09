# Tutorial ELT Pipeline Example

Source files referenced by `docs/guide/tutorial_elt_pipeline.md`.

Run the business-logic script directly:

```bash
uv run python run.py
```

Inspect the Python task contract and validate the workflow:

```bash
cargo run -p czar-cli -- schema export tasks_load_bronze.py
cargo run -p czar-cli -- validate-workflow workflow.yaml
```

Run the workflow locally or deploy it to the local control plane:

```bash
cargo run -p czar-cli -- run workflow.yaml
cargo run -p czar-cli -- workflow deploy workflow.yaml
```
