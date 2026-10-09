"""Static parallelization example.

`process_shard` runs as 4 parallel instances. Each reads PARALLEL_INDEX and
PARALLEL_COUNT from the environment and processes its slice of the dataset.

`aggregate_results` collects all shard outputs, which Czar passes as a list
ordered by index.
"""

import os
from typing import TypedDict

DATASET = list(range(100))  # 100 items to distribute across shards


class ShardResult(TypedDict):
    shard: int
    total: int
    item_count: int


class AggregateResult(TypedDict):
    grand_total: int
    shard_count: int
    item_count: int


def process_shard() -> ShardResult:
    index = int(os.environ.get("parallel_index", 0))
    count = int(os.environ.get("parallel_count", 1))

    # Distribute items across shards by modulo
    my_items = [x for i, x in enumerate(DATASET) if i % count == index]
    total = sum(my_items)

    print(f"Shard {index}/{count}: {len(my_items)} items, sum={total}")
    return {"shard": index, "total": total, "item_count": len(my_items)}


def aggregate_results(shards: list[ShardResult]) -> AggregateResult:
    grand_total = sum(s["total"] for s in shards)
    item_count = sum(s["item_count"] for s in shards)

    print(f"{len(shards)} shards complete — grand total: {grand_total}, items: {item_count}")
    return {"grand_total": grand_total, "shard_count": len(shards), "item_count": item_count}
