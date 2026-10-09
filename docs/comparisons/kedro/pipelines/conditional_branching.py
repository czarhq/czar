import time
from typing import Any

from kedro.pipeline import Pipeline, node

HIGH_QUALITY_THRESHOLD = 0.8


def check_data_quality() -> float:
    time.sleep(1)
    score = 0.6
    print(f"Data quality score: {score}")
    return score


def process_based_on_quality(quality_score: float) -> dict[str, Any]:
    if quality_score > HIGH_QUALITY_THRESHOLD:
        time.sleep(1)
        print("Processing high quality data")
        return {"quality": "high", "processed": True}
    time.sleep(1)
    print("Cleaning low quality data")
    time.sleep(1)
    print("Processing cleaned low quality data")
    return {"quality": "low", "processed": True}


def get_partitions() -> list[str]:
    return ["partition_1", "partition_2", "partition_3"]


def process_partition_1() -> str:
    time.sleep(1)
    print("Processing partition_1")
    return "partition_1_done"


def process_partition_2() -> str:
    time.sleep(1)
    print("Processing partition_2")
    return "partition_2_done"


def process_partition_3() -> str:
    time.sleep(1)
    print("Processing partition_3")
    return "partition_3_done"


def combine_results(p1: str, p2: str, p3: str) -> None:
    time.sleep(1)
    print(f"Combined all partition results: {p1}, {p2}, {p3}")


def create_pipeline(**_kwargs: Any) -> Pipeline:
    # Quality-based processing (conditional handled internally)
    quality_pipeline = Pipeline(
        [
            node(
                func=check_data_quality,
                inputs=None,
                outputs="quality_score",
                name="check_quality",
            ),
            node(
                func=process_based_on_quality,
                inputs="quality_score",
                outputs="processed_data",
                name="process_quality",
            ),
        ]
    )

    # Partition processing (manually enumerated - no dynamic generation)
    partition_pipeline = Pipeline(
        [
            node(
                func=process_partition_1,
                inputs=None,
                outputs="p1_result",
                name="process_partition_1",
            ),
            node(
                func=process_partition_2,
                inputs=None,
                outputs="p2_result",
                name="process_partition_2",
            ),
            node(
                func=process_partition_3,
                inputs=None,
                outputs="p3_result",
                name="process_partition_3",
            ),
            node(
                func=combine_results,
                inputs=["p1_result", "p2_result", "p3_result"],
                outputs=None,
                name="combine_results",
            ),
        ]
    )

    return quality_pipeline + partition_pipeline


if __name__ == "__main__":
    from kedro.io import DataCatalog
    from kedro.runner import SequentialRunner

    pipeline = create_pipeline()
    catalog = DataCatalog()
    runner = SequentialRunner()
    runner.run(pipeline, catalog=catalog)
