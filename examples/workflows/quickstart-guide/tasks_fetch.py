# quickstart-guide/tasks_fetch.py
import os

from czar import Context, task
from pydantic import BaseModel
from src.data_ops import fetch_data

class Output(BaseModel):
    data: str

@task
def main(url: str, ctx: Context) -> Output:
    source_label = os.getenv("SOURCE_LABEL", "source")
    ctx.log(f"Fetching {source_label} from {url}")
    data = fetch_data(url)
    return Output(data=data)
