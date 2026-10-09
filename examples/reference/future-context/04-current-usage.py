from czar import Context, task


@task
def main(url: str, ctx: Context) -> dict:
    ctx.log.info(f"Fetching {url}")
    ctx.metrics.record("fetch_started", 1, labels={"task": ctx.task_name})

    data = fetch(url)

    ctx.progress(1, 1, message="done")
    return {"size": len(data)}
