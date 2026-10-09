import time
from datetime import UTC, datetime

from airflow.sdk import TaskGroup, dag, task

QUALITY_THRESHOLD = 0.8


@dag(
    dag_id="conditional_branching_flow",
    start_date=datetime(2024, 1, 1, tzinfo=UTC),
    schedule=None,
    catchup=False,
)
def conditional_branching_flow() -> None:
    quality = check_data_quality()
    branch = decide_processing_path(quality)  # type: ignore

    high_quality = process_high_quality()

    with TaskGroup("handle_low_quality") as low_quality_group:
        cleaned = clean_data()
        process_low_quality() << cleaned  # type: ignore

    branch >> [high_quality, low_quality_group]  # type: ignore

    partitions = get_partitions()
    processed = process_partition.expand(partition=partitions)  # type: ignore
    [high_quality, low_quality_group] >> partitions  # type: ignore
    processed >> combine_results()  # type: ignore


@task
def check_data_quality() -> float:
    time.sleep(1)
    score = 0.6
    print(f"Data quality score: {score}")
    return score


@task.branch
def decide_processing_path(quality_score: float) -> str:
    if quality_score > QUALITY_THRESHOLD:
        return "process_high_quality"
    return "handle_low_quality.clean_data"


@task
def process_high_quality() -> None:
    time.sleep(1)
    print("Processing high quality data")


@task
def clean_data() -> None:
    time.sleep(1)
    print("Cleaning low quality data")


@task
def process_low_quality() -> None:
    time.sleep(1)
    print("Processing cleaned low quality data")


@task
def get_partitions() -> list[str]:
    return ["partition_1", "partition_2", "partition_3"]


@task
def process_partition(partition: str) -> None:
    time.sleep(1)
    print(f"Processing {partition}")


@task
def combine_results() -> None:
    time.sleep(1)
    print("Combined all partition results")


conditional_branching_flow()
