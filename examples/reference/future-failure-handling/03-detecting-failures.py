# tasks/notify.py
from czar import task, Context
from pydantic import BaseModel

class ExtractOutput(BaseModel):
    status: str

class TransformOutput(BaseModel):
    status: str

class LoadOutput(BaseModel):
    status: str

class Input(BaseModel):
    extract: ExtractOutput | None = None
    transform: TransformOutput | None = None
    load: LoadOutput | None = None

class Output(BaseModel):
    notified: bool

@task
def main(input: Input, ctx: Context) -> Output:
    if input.load is None:
        # Load failed, send failure notification
        send_alert(f"Workflow {ctx.workflow_name} failed")
    else:
        # Success, send success notification
        send_notification(f"Workflow {ctx.workflow_name} completed")

    return Output(notified=True)
