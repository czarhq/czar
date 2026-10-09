import sqlite3
from pathlib import Path
from typing import TypedDict

from czar import Context, task


class PrepareDbOutput(TypedDict):
    db_path: str


class CustomerRow(TypedDict):
    customer: str
    total: float


class QueryResult(TypedDict):
    rows: list[CustomerRow]
    row_count: int
    sql: str


class SummaryOutput(TypedDict):
    top_customer: str
    row_count: int


@task
def prepare_db(ctx: Context) -> PrepareDbOutput:
    db_path = Path("warehouse.db").resolve()
    with sqlite3.connect(db_path) as conn:
        conn.execute("drop table if exists orders")
        conn.execute(
            "create table orders (customer text not null, amount real not null)"
        )
        conn.executemany(
            "insert into orders(customer, amount) values(?, ?)",
            [
                ("alice", 120.0),
                ("alice", 30.0),
                ("bob", 95.0),
                ("carol", 210.0),
                ("dave", 45.0),
            ],
        )
        conn.commit()
    ctx.log.info(f"prepared sqlite database at {db_path}")
    return {"db_path": str(db_path)}


@task
def summarize(results: QueryResult, ctx: Context) -> SummaryOutput:
    rows = results["rows"]
    top_customer = rows[0]["customer"] if rows else "none"
    ctx.log.info(f"top customer: {top_customer}")
    return {
        "top_customer": top_customer,
        "row_count": int(results["row_count"]),
    }
