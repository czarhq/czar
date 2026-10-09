@task
def main(ctx: Context) -> dict:
    count = ctx.parallel_count or 1
    index = ctx.parallel_index or 0
    items = [item for i, item in enumerate(load_items()) if i % count == index]
    return {"count": len(items)}
