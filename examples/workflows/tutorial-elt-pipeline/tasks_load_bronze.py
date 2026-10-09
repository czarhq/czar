# tasks_load_bronze.py
from czar import Context, task
from src_pipeline import load_to_bronze


@task
def main(source: str, date: str, data_dir: str, ctx: Context) -> int:
    ctx.log(f"Loading bronze layer for {source} on {date}")
    return load_to_bronze(source, date, data_dir)
