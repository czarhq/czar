# executors/bigquery.py
from czar import executor, Context
from pydantic import BaseModel
from google.cloud import bigquery
from pathlib import Path
from jinja2 import Template
import yaml

class BigQueryConfig(BaseModel):
    project: str
    location: str = "US"
    cost_per_tb: float = 5.0

class TaskConfig(BaseModel):
    file: str
    params: dict[str, str | int | float | bool] = {}

class TaskOutput(BaseModel):
    rows_affected: int
    bytes_processed: int
    estimated_cost_usd: float

@executor
class BigQueryExecutor:
    def __init__(self, config: BigQueryConfig):
        self.client = bigquery.Client(
            project=config.project,
            location=config.location
        )
        self.cost_per_tb = config.cost_per_tb

    def run(self, config_file: str, ctx: Context, upstream: dict[str, object] = {}) -> TaskOutput:
        task_config = TaskConfig(**yaml.safe_load(Path(config_file).read_text()))

        sql_template = Path(task_config.file).read_text()
        template = Template(sql_template)
        context = {**task_config.params, **upstream}
        sql = template.render(**context)

        job = self.client.query(sql)
        result = job.result()

        bytes_processed = job.total_bytes_processed or 0
        tb_processed = bytes_processed / (1024 ** 4)
        cost = tb_processed * self.cost_per_tb

        ctx.metrics.record("bytes_processed", bytes_processed, labels={
            "task": ctx.task_name,
            "workflow": ctx.workflow_name,
        })

        return TaskOutput(
            rows_affected=result.total_rows or 0,
            bytes_processed=bytes_processed,
            estimated_cost_usd=round(cost, 4)
        )
