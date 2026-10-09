"""Python tasks for the conditional_branching benchmark."""
import time
from typing import TypedDict


class QualityScore(TypedDict):
    score: float


class Partitions(TypedDict):
    partitions: list[str]


def check_data_quality() -> QualityScore:
    time.sleep(1)
    score = 0.6
    print(f"Data quality score: {score}")
    return {"score": score}


def process_high_quality() -> None:
    time.sleep(1)
    print("Processing high quality data")


def clean_data() -> None:
    time.sleep(1)
    print("Cleaning low quality data")


def process_low_quality() -> None:
    time.sleep(1)
    print("Processing cleaned low quality data")


def get_partitions() -> Partitions:
    return {"partitions": ["partition_1", "partition_2", "partition_3"]}


def process_partition(partition: str) -> None:
    time.sleep(1)
    print(f"Processing {partition}")


def combine_results() -> None:
    time.sleep(1)
    print("Combined all partition results")
