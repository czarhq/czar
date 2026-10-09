from czar import task, Context
from pydantic import BaseModel

class Input(BaseModel):
    item: str

class Output(BaseModel):
    processed: int

@task
def main(input: Input, ctx: Context) -> Output:
    rows = process(input.item)

    # Logging
    ctx.log.info(f"Processed {rows} rows")
    ctx.log.warning("Memory usage high")
    ctx.log.error("Validation failed")

    # Metrics
    ctx.metrics.record("rows_processed", rows)
    ctx.metrics.record("memory_mb", 512, labels={
        "task": ctx.task_name,
        "attempt": ctx.attempt
    })

    return Output(processed=rows)
