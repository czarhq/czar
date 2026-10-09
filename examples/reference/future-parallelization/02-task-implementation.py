# tasks/process.py

def main(input, ctx):
    # Divide work by index
    all_items = get_all_items()
    my_items = [
        item for i, item in enumerate(all_items)
        if i % ctx.parallel_count == ctx.parallel_index
    ]
    results = process_items(my_items)
    return {"processed": len(results)}
