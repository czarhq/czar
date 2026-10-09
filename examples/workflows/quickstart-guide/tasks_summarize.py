# quickstart-guide/tasks_summarize.py
from czar import Context, task
from pydantic import BaseModel
from src.data_ops import summarize_data

class FetchOutput(BaseModel):
    data: str

class Output(BaseModel):
    total_chars: int
    total_lines: int
    non_empty_lines: int

@task
def main(fetch: FetchOutput, ctx: Context) -> Output:
    ctx.log("Summarizing fetched payload")
    stats = summarize_data(fetch.data)
    return Output(**stats)
