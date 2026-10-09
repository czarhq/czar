import time
from typing import Any

from metaflow.decorators import step
from metaflow.flowspec import FlowSpec

QUALITY_THRESHOLD = 0.8


class ConditionalBranchingFlow(FlowSpec):
    quality_score: float
    processing_path: str
    partitions: list[str]

    @step
    def start(self) -> None:
        self.next(self.check_data_quality)

    @step
    def check_data_quality(self) -> None:
        time.sleep(1)
        self.quality_score = 0.6
        print(f"Data quality score: {self.quality_score}")
        self.next(self.process_based_on_quality)

    @step
    def process_based_on_quality(self) -> None:
        time.sleep(1)
        if self.quality_score > QUALITY_THRESHOLD:
            print("Processing high quality data")
            self.processing_path = "high_quality"
        else:
            print("Cleaning low quality data")
            time.sleep(1)
            print("Processing cleaned low quality data")
            self.processing_path = "low_quality"
        self.next(self.get_partitions)

    @step
    def get_partitions(self) -> None:
        self.partitions = ["partition_1", "partition_2", "partition_3"]
        self.next(
            self.process_partition, foreach="partitions"
        )

    @step
    def process_partition(self) -> None:
        time.sleep(1)
        print(f"Processing {self.input}")
        self.next(self.combine_results)

    @step
    def combine_results(self, inputs: Any) -> None:
        _ = inputs
        time.sleep(1)
        print("Combined all partition results")
        self.next(self.end)

    @step
    def end(self) -> None:
        print("Workflow complete")


if __name__ == "__main__":
    ConditionalBranchingFlow()
