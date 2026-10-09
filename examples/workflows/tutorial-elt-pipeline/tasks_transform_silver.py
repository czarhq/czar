# tasks_transform_silver.py
from czar import Context, task
from src_pipeline import transform_to_silver


@task
def main(source: str, data_dir: str, ctx: Context) -> int:
    ctx.log(f"Transforming silver layer for {source}")
    return transform_to_silver(source, data_dir)
