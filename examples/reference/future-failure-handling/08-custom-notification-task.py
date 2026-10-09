# tasks/notify.py
@task
def main(input: Input, ctx: Context) -> Output:
    if any(output is None for output in [input.extract, input.transform, input.load]):
        send_pagerduty_alert(
            severity="error",
            message=f"Workflow {ctx.workflow_name} failed",
            run_id=ctx.run_id
        )
    return Output(notified=True)
