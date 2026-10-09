@task
def main(input: Input, ctx: Context) -> Output:
    # Divide work across instances
    all_items = get_all_items()
    my_items = [
        item for i, item in enumerate(all_items)
        if i % ctx.parallel_count == ctx.parallel_index
    ]

    results = [process(item) for item in my_items]

    ctx.log.info(
        f"Instance {ctx.parallel_index}/{ctx.parallel_count} "
        f"processed {len(results)} items"
    )

    return Output(results=results)
