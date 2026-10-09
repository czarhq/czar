import time
from typing import Any

from mage_ai.data_preparation.decorators import (
    custom,
    data_exporter,
    data_loader,
)

HIGH_QUALITY_THRESHOLD = 0.8


@data_loader
def check_data_quality(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
    time.sleep(1)
    score = 0.6
    print(f"Data quality score: {score}")
    return {"score": score, "is_high_quality": score > HIGH_QUALITY_THRESHOLD}


@custom
def process_by_quality(
    quality_data: dict[str, Any],
    *_args: Any,
    **_kwargs: Any,
) -> dict[str, Any]:
    time.sleep(1)

    if quality_data["is_high_quality"]:
        print("Processing high quality data")
        result = {"path": "high_quality", "processed": True}
    else:
        print("Cleaning low quality data")
        time.sleep(1)
        print("Processing cleaned low quality data")
        result = {"path": "low_quality", "processed": True, "cleaned": True}

    return result


@data_loader
def get_partitions(*_args: Any, **_kwargs: Any) -> list[str]:
    return ["partition_1", "partition_2", "partition_3"]


@custom
def process_partitions(
    _processing_result: dict[str, Any],
    partitions: list[str],
    *_args: Any,
    **_kwargs: Any,
) -> list[dict[str, str]]:
    results: list[dict[str, str]] = []
    for partition in partitions:
        time.sleep(1)
        print(f"Processing {partition}")
        results.append({"partition": partition, "status": "processed"})

    return results


@data_exporter
def combine_results(
    partition_results: list[dict[str, str]],
    *_args: Any,
    **_kwargs: Any,
) -> None:
    time.sleep(1)
    print(f"Combined {len(partition_results)} partition results")
    print(f"Results: {partition_results}")
