import csv
from pathlib import Path

import duckdb

from czar import Context, task


ROOT = Path(__file__).resolve().parent
SEED_PATH = ROOT / "transform" / "seeds" / "raw_orders.csv"
WAREHOUSE_PATH = ROOT / "warehouse.duckdb"


@task
def prepare_seed(ctx: Context) -> dict[str, object]:
    rows = [
        {"order_id": 1, "customer_id": 101, "ordered_at": "2026-03-25", "amount": 42.50},
        {"order_id": 2, "customer_id": 101, "ordered_at": "2026-03-25", "amount": 17.25},
        {"order_id": 3, "customer_id": 202, "ordered_at": "2026-03-26", "amount": 73.00},
        {"order_id": 4, "customer_id": 303, "ordered_at": "2026-03-26", "amount": 11.00},
        {"order_id": 5, "customer_id": 202, "ordered_at": "2026-03-27", "amount": 64.40},
    ]
    SEED_PATH.parent.mkdir(parents=True, exist_ok=True)
    with SEED_PATH.open("w", newline="") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=["order_id", "customer_id", "ordered_at", "amount"],
        )
        writer.writeheader()
        writer.writerows(rows)
    ctx.log.info(f"wrote seed data to {SEED_PATH}")
    return {"seed_path": str(SEED_PATH), "row_count": len(rows)}


@task
def publish_report(ctx: Context) -> dict[str, object]:
    query = """
        select
            cast(order_day as varchar) as order_day,
            total_revenue,
            order_count
        from daily_revenue
        order by order_day
    """
    with duckdb.connect(str(WAREHOUSE_PATH), read_only=True) as conn:
        rows = conn.execute(query).fetchall()
    rendered = [
        {
            "order_day": order_day,
            "total_revenue": float(total_revenue),
            "order_count": int(order_count),
        }
        for order_day, total_revenue, order_count in rows
    ]
    ctx.log.info(f"daily_revenue rows: {rendered}")
    return {"warehouse_path": str(WAREHOUSE_PATH), "rows": rendered}
