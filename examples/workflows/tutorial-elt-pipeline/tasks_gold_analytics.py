# tasks_gold_analytics.py
from czar import Context, task
from src_pipeline import create_gold_analytics


@task
def main(data_dir: str, ctx: Context) -> dict:
    ctx.log("Creating gold analytics tables")
    return create_gold_analytics(data_dir)
