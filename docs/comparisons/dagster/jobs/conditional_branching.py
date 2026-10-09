import time
from collections.abc import Generator
from typing import Any

from dagster import DynamicOut, DynamicOutput, Out, job, op

QUALITY_THRESHOLD = 0.8


@op
def check_data_quality() -> float:
    time.sleep(1)
    score = 0.6
    print(f"Data quality score: {score}")
    return score


@op(out=Out(is_required=False))
def process_high_quality(quality_score: float) -> bool | None:
    if quality_score > QUALITY_THRESHOLD:
        time.sleep(1)
        print("Processing high quality data")
        return True
    return None


@op(out=Out(is_required=False))
def clean_data(quality_score: float) -> bool | None:
    if quality_score <= QUALITY_THRESHOLD:
        time.sleep(1)
        print("Cleaning low quality data")
        return True
    return None


@op(out=Out(is_required=False))
def process_low_quality(cleaned: bool | None) -> bool | None:
    if cleaned is not None:
        time.sleep(1)
        print("Processing cleaned low quality data")
        return True
    return None


@op(out=DynamicOut())
def get_partitions() -> Generator[DynamicOutput[str], None, None]:
    partitions = ["partition_1", "partition_2", "partition_3"]
    for partition in partitions:
        yield DynamicOutput(partition, mapping_key=partition)


@op
def process_partition(partition: str) -> str:
    time.sleep(1)
    print(f"Processing {partition}")
    return partition


@op
def combine_results(partition_results: list[Any]) -> None:
    time.sleep(1)
    print(f"Combined {len(partition_results)} partition results")


@job
def conditional_branching_flow() -> None:
    quality = check_data_quality()
    process_high_quality(quality)
    cleaned = clean_data(quality)
    process_low_quality(cleaned)

    partitions = get_partitions()
    processed = partitions.map(process_partition)
    combine_results(processed.collect())


