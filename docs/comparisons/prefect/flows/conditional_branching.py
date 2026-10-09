import time

from prefect import flow, task

QUALITY_THRESHOLD = 0.8


@flow
def conditional_branching_flow() -> None:
    quality = check_data_quality()

    if quality > QUALITY_THRESHOLD:
        process_high_quality()
    else:
        clean_data()
        process_low_quality()

    partitions = get_partitions()
    for partition in partitions:
        process_partition(partition)

    combine_results()


@task
def check_data_quality() -> float:
    time.sleep(1)
    score = 0.6
    print(f"Data quality score: {score}")
    return score


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


if __name__ == "__main__":
    conditional_branching_flow()
