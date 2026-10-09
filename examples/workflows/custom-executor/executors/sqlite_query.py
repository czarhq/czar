from __future__ import annotations

import json
import sqlite3
from pathlib import Path

from czar import Context, executor


@executor
class SqliteQueryExecutor:
    def __init__(self, config: dict):
        self.default_limit = int(config.get("default_limit", 100))

    def run(self, config_file: str, ctx: Context, task_input: dict) -> dict:
        config = json.loads(Path(config_file).read_text())
        sql = config["sql"].format(limit=self.default_limit, **task_input)
        with sqlite3.connect(task_input["db_path"]) as conn:
            cursor = conn.execute(sql)
            columns = [column[0] for column in cursor.description or []]
            rows = [
                dict(zip(columns, row, strict=False))
                for row in cursor.fetchall()
            ]

        ctx.log.info(f"executed query with {len(rows)} row(s)")
        ctx.metrics.record("row_count", len(rows), labels={"executor": "sqlite_query"})
        return {
            "rows": rows,
            "row_count": len(rows),
            "sql": sql,
        }
