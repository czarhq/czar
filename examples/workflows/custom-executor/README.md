# Custom Executor Example

This example registers a custom Python executor in `czar.yaml`, then uses it as a first-class task in `workflow.yaml`.

It does three things:

1. `prepare_db` creates a local SQLite database with sample order data.
2. `query_top_customers` runs through the custom `sqlite_query` executor and returns structured rows.
3. `summarize` consumes that output as a normal Czar Python task.

Run it with:

```bash
just example custom-executor
```
