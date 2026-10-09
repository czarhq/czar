# Tutorial: ELT Pipeline with Medallion Architecture

_Updated at: 2026-10-09_

This tutorial adds orchestration to an ELT pipeline with bronze, silver, and gold layers.

## Examples Covered

1. [Tutorial ELT pipeline example](../../examples/workflows/tutorial-elt-pipeline/)

## Your Existing Code

Say you have an ELT pipeline. It loads raw data from an API into bronze tables, cleans it into silver tables, and joins the silver tables into gold tables. Each source has its own DuckDB file, because DuckDB lets only one process write to a file at a time.

<!-- doc-snippet: path=examples/workflows/tutorial-elt-pipeline/docs/src_pipeline.py sha256=2152c5fefde87a6459e8c2b79b4f857fc2653cfbd142482e6573bc3e7d01a499 lines=1-182 -->
```py
# src_pipeline.py
import json
import tempfile
from pathlib import Path

import duckdb
import requests


def load_to_bronze(source: str, date: str, data_dir: str) -> int:
    """Load raw JSONPlaceholder data into the source's bronze table."""
    records = _fetch_records(source)

    Path(data_dir).mkdir(parents=True, exist_ok=True)
    con = duckdb.connect(_source_db(data_dir, source))
    _prepare_database(con)

    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as handle:
        json.dump(records, handle)
        temp_path = handle.name

    try:
        con.execute(
            f"""
            CREATE OR REPLACE TABLE bronze.{source} AS
            SELECT
                *,
                TIMESTAMP '{date} 00:00:00' AS loaded_at,
                DATE '{date}' AS batch_date
            FROM read_json_auto(?)
            """,
            [temp_path],
        )
        count = con.execute(f"SELECT COUNT(*) FROM bronze.{source}").fetchone()[0]
    finally:
        con.close()
        Path(temp_path).unlink(missing_ok=True)

    return count


def transform_to_silver(source: str, data_dir: str) -> int:
    """Normalize the source's bronze table into a cleaned silver table."""
    con = duckdb.connect(_source_db(data_dir, source))
    _prepare_database(con)

    if source == "posts":
        con.execute(
            """
            CREATE OR REPLACE TABLE silver.posts AS
            SELECT
                CAST(id AS INTEGER) AS post_id,
                CAST(userId AS INTEGER) AS user_id,
                TRIM(title) AS title,
                TRIM(body) AS body,
                loaded_at,
                batch_date
            FROM bronze.posts
            WHERE id IS NOT NULL
            """
        )
    elif source == "comments":
        con.execute(
            """
            CREATE OR REPLACE TABLE silver.comments AS
            SELECT
                CAST(id AS INTEGER) AS comment_id,
                CAST(postId AS INTEGER) AS post_id,
                TRIM(name) AS name,
                LOWER(TRIM(email)) AS email,
                TRIM(body) AS body,
                loaded_at,
                batch_date
            FROM bronze.comments
            WHERE id IS NOT NULL
            """
        )
    elif source == "users":
        con.execute(
            """
            CREATE OR REPLACE TABLE silver.users AS
            SELECT
                CAST(id AS INTEGER) AS user_id,
                TRIM(name) AS name,
                TRIM(username) AS username,
                LOWER(TRIM(email)) AS email,
                loaded_at,
                batch_date
            FROM bronze.users
            WHERE id IS NOT NULL
            """
        )
    else:
        raise ValueError(f"Unsupported source '{source}'")

    result = con.execute(f"SELECT COUNT(*) FROM silver.{source}").fetchone()[0]
    con.close()
    return result


def create_gold_analytics(data_dir: str) -> dict:
    """Join the sources' silver tables into gold tables; return summary metrics."""
    con = duckdb.connect(str(Path(data_dir) / "warehouse.duckdb"))
    _prepare_database(con)
    for source in ["users", "posts", "comments"]:
        con.execute(f"ATTACH '{_source_db(data_dir, source)}' AS {source}_db (READ_ONLY)")

    con.execute(
        """
        CREATE OR REPLACE TABLE gold.user_engagement AS
        SELECT
            u.user_id,
            u.name,
            u.username,
            COUNT(DISTINCT p.post_id) AS post_count,
            COUNT(DISTINCT c.comment_id) AS comment_count,
            COUNT(DISTINCT c.comment_id) * 1.0 / NULLIF(COUNT(DISTINCT p.post_id), 0) AS avg_comments_per_post
        FROM users_db.silver.users u
        LEFT JOIN posts_db.silver.posts p ON u.user_id = p.user_id
        LEFT JOIN comments_db.silver.comments c ON p.post_id = c.post_id
        GROUP BY u.user_id, u.name, u.username
        """
    )

    con.execute(
        """
        CREATE OR REPLACE TABLE gold.top_posts AS
        SELECT
            p.post_id,
            p.title,
            u.name AS author,
            COUNT(c.comment_id) AS comment_count
        FROM posts_db.silver.posts p
        JOIN users_db.silver.users u ON p.user_id = u.user_id
        LEFT JOIN comments_db.silver.comments c ON p.post_id = c.post_id
        GROUP BY p.post_id, p.title, u.name
        ORDER BY comment_count DESC
        LIMIT 10
        """
    )

    stats = con.execute(
        """
        SELECT
            (SELECT COUNT(*) FROM users_db.silver.users) AS total_users,
            (SELECT COUNT(*) FROM posts_db.silver.posts) AS total_posts,
            (SELECT COUNT(*) FROM comments_db.silver.comments) AS total_comments,
            (SELECT AVG(comment_count) FROM gold.user_engagement) AS avg_comments_per_user
        """
    ).fetchone()
    con.close()

    return {
        "total_users": stats[0],
        "total_posts": stats[1],
        "total_comments": stats[2],
        "avg_comments_per_user": round(stats[3] or 0.0, 2),
    }


def _fetch_records(source: str) -> list[dict]:
    response = requests.get(
        f"https://jsonplaceholder.typicode.com/{source}",
        timeout=30,
    )
    response.raise_for_status()
    records = response.json()
    if not isinstance(records, list):
        raise ValueError(f"Expected a list of records for source '{source}'")
    return records


def _source_db(data_dir: str, source: str) -> str:
    # One DuckDB file per source: DuckDB lets one process write a file at a
    # time, so the sources can load in parallel only into separate files.
    return str(Path(data_dir) / f"{source}.duckdb")


def _prepare_database(con: duckdb.DuckDBPyConnection) -> None:
    con.execute("CREATE SCHEMA IF NOT EXISTS bronze")
    con.execute("CREATE SCHEMA IF NOT EXISTS silver")
    con.execute("CREATE SCHEMA IF NOT EXISTS gold")
```

You run it as a script:

<!-- doc-snippet: path=examples/workflows/tutorial-elt-pipeline/run.py sha256=9e73ab5bed2f7d568470290e3554ca213438375d446e79a35562e1f81be9f7cd lines=1-14 -->
```py
from src_pipeline import create_gold_analytics, load_to_bronze, transform_to_silver

DATA_DIR = "data"

for source in ["posts", "comments", "users"]:
    bronze_count = load_to_bronze(source, "2024-01-15", DATA_DIR)
    print(f"Loaded {bronze_count} records to bronze.{source}")

    silver_count = transform_to_silver(source, DATA_DIR)
    print(f"Transformed {silver_count} records to silver.{source}")

# After all sources loaded and transformed
stats = create_gold_analytics(DATA_DIR)
print(f"Analytics: {stats['total_users']} users, {stats['total_posts']} posts, {stats['total_comments']} comments")
```

**Problems**:
- The three sources load one after another, although they don't depend on each other
- A network error stops the whole script; nothing is retried
- Nothing runs it on a schedule
- When it fails, you find the failed step in the terminal output

## Add Orchestration

Three steps: wrap the functions in tasks, describe the order in a workflow, and run it.

### 1. Wrap Your Functions in Tasks

Each task is a small file that calls one of your functions. The functions in `src_pipeline.py` don't change and don't import Czar.

The data stays in DuckDB, so the tasks don't pass it to each other. Each task returns a row count or a summary, and the workflow decides the order.

<!-- doc-snippet: path=examples/workflows/tutorial-elt-pipeline/tasks_load_bronze.py sha256=853fd7cbde73b9cabc19a032054347dcd744f97031692ba355bd6bde02af78b2 lines=1-9 -->
```py
# tasks_load_bronze.py
from czar import Context, task
from src_pipeline import load_to_bronze


@task
def main(source: str, date: str, data_dir: str, ctx: Context) -> int:
    ctx.log(f"Loading bronze layer for {source} on {date}")
    return load_to_bronze(source, date, data_dir)
```

<!-- doc-snippet: path=examples/workflows/tutorial-elt-pipeline/tasks_transform_silver.py sha256=c89c7a68d008558b324218e596021a2698b2dcd8d544aa6d0bb6d1cefcd759b4 lines=1-9 -->
```py
# tasks_transform_silver.py
from czar import Context, task
from src_pipeline import transform_to_silver


@task
def main(source: str, data_dir: str, ctx: Context) -> int:
    ctx.log(f"Transforming silver layer for {source}")
    return transform_to_silver(source, data_dir)
```

<!-- doc-snippet: path=examples/workflows/tutorial-elt-pipeline/tasks_gold_analytics.py sha256=72610f4780b1fc6b44c8439ee1f9b00d59db6e76713f0b366c4214a11e407f8a lines=1-9 -->
```py
# tasks_gold_analytics.py
from czar import Context, task
from src_pipeline import create_gold_analytics


@task
def main(data_dir: str, ctx: Context) -> dict:
    ctx.log("Creating gold analytics tables")
    return create_gold_analytics(data_dir)
```

`@task` marks `main` as the task's entry point. Its parameters are the task's inputs, and `ctx.log()` writes a line to the task's log.

### 2. Define the Workflow

<!-- doc-snippet: path=examples/workflows/tutorial-elt-pipeline/workflow.yaml sha256=059262a31f61c14fa050514ab1da33fcaf0dd519625dca454a42b6e1a01fda55 lines=1-104 -->
```yaml
# workflow.yaml
name: elt_pipeline
schedule: "0 2 * * *"

tasks:
  bronze_posts:
    executor: python
    file: tasks_load_bronze.py
    input_type:
      source: str
      date: str
      data_dir: str
    inputs:
      source:
        const: posts
      date:
        const: "2024-01-15"
      data_dir:
        const: data
    timeout: 600
    retries: 3

  bronze_comments:
    executor: python
    file: tasks_load_bronze.py
    input_type:
      source: str
      date: str
      data_dir: str
    inputs:
      source:
        const: comments
      date:
        const: "2024-01-15"
      data_dir:
        const: data
    timeout: 600
    retries: 3

  bronze_users:
    executor: python
    file: tasks_load_bronze.py
    input_type:
      source: str
      date: str
      data_dir: str
    inputs:
      source:
        const: users
      date:
        const: "2024-01-15"
      data_dir:
        const: data
    timeout: 600
    retries: 3

  silver_posts:
    executor: python
    file: tasks_transform_silver.py
    input_type:
      source: str
      data_dir: str
    inputs:
      source:
        const: posts
      data_dir:
        const: data
    depends_on: [bronze_posts]

  silver_comments:
    executor: python
    file: tasks_transform_silver.py
    input_type:
      source: str
      data_dir: str
    inputs:
      source:
        const: comments
      data_dir:
        const: data
    depends_on: [bronze_comments]

  silver_users:
    executor: python
    file: tasks_transform_silver.py
    input_type:
      source: str
      data_dir: str
    inputs:
      source:
        const: users
      data_dir:
        const: data
    depends_on: [bronze_users]

  gold_analytics:
    executor: python
    file: tasks_gold_analytics.py
    input_type:
      data_dir: str
    inputs:
      data_dir:
        const: data
    depends_on: [silver_posts, silver_comments, silver_users]
```

What happens:
- `schedule: "0 2 * * *"` starts a run every day at 02:00 UTC, while `czar serve` is running.
- The three `bronze_*` tasks start together. A bronze task that fails is retried up to 3 times (`retries: 3`).
- Each `silver_*` task starts as soon as its own bronze task succeeds.
- `gold_analytics` waits for all three silver tasks.
- If a bronze task still fails, its silver task and `gold_analytics` are marked failed. The other sources still load.

The date is fixed at 2024-01-15, so every run loads the same day. That keeps the example repeatable.

### 3. Run It

Put the files in one folder, as a Python project with czar and your pipeline's libraries:

```bash
uv init elt-pipeline && cd elt-pipeline
uv add czar duckdb requests
# then add src_pipeline.py, the three tasks_*.py files and workflow.yaml
```

Run it once, in the terminal:

```bash
czar run workflow.yaml
```

Czar prints each task's log as it runs, then a table with every task's status, duration and attempts.

To have the schedule run it, deploy it and keep the server running:

```bash
czar workflow deploy workflow.yaml
czar serve
```

Then follow the runs in the web UI at <http://127.0.0.1:4000>, in the terminal UI (`czar-tui`), or with the CLI:

```bash
czar status --workflow elt_pipeline   # recent runs and their IDs
czar logs <run-id> bronze_posts       # one task's log
```

The web UI and `czar-tui` can also start, pause, resume and cancel runs.

## Key Takeaways

1. **Your code stays separate**: the functions in `src_pipeline.py` don't know about Czar
2. **In ELT, the data stays in the database**: tasks only need `depends_on`. To pass values from one task to the next, see [Tasks](../design/tasks.md) and [Type Schemas](../design/type_schemas.md)
3. **The schedule and retries are lines in the workflow file**, not code

## Next Steps

- [Tasks](../design/tasks.md) - Writing task wrappers in detail
- [Context](../design/context.md) - Runtime task metadata and helpers
- [Workflows](../design/workflows.md) - DAG definitions and dependency wiring
- [Failure Handling](../design/failure_handling.md) - Retries, hooks, cancellation, notifications, and skip behavior
- [Operations](../design/operations.md#scheduling) - Deploying, scheduling, and inspecting workflows
