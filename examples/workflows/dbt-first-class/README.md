# dbt First-Class Example

This example shows dbt imported as first-class Czar tasks.

What it includes:

- `prepare_seed` writes the source CSV before dbt starts
- Czar imports the dbt project from `./transform`
- dbt seed/model/test nodes become ordinary Czar tasks
- `publish_report` depends on specific dbt nodes and reads the DuckDB output
- the example ships a tiny repo-owned `dbt` CLI shim under `bin/dbt`, so it stays runnable without pulling the full Python dbt dependency stack into the repo-managed license set

Useful commands from this directory:

```bash
uv sync
just example dbt-first-class
```
