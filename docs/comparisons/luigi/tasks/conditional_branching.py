import json
import time
from typing import Any

import luigi

QUALITY_THRESHOLD = 0.8


class CheckDataQuality(luigi.Task):
    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("quality_score.json")

    def run(self) -> None:
        time.sleep(1)
        score = 0.6
        print(f"Data quality score: {score}")
        with self.output().open("w") as f:
            json.dump({"score": score}, f)


class ProcessHighQuality(luigi.Task):
    def requires(self) -> CheckDataQuality:
        return CheckDataQuality()

    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("high_quality_processed.done")

    def run(self) -> None:
        with self.input().open("r") as f:  # type: ignore
            quality_data: dict[str, Any] = json.load(f)

        if quality_data["score"] > QUALITY_THRESHOLD:
            time.sleep(1)
            print("Processing high quality data")
            with self.output().open("w") as f:
                f.write("done")
        else:
            with self.output().open("w") as f:
                f.write("skipped")


class CleanData(luigi.Task):
    def requires(self) -> CheckDataQuality:
        return CheckDataQuality()

    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("cleaned.done")

    def run(self) -> None:
        with self.input().open("r") as f:  # type: ignore
            quality_data: dict[str, Any] = json.load(f)

        if quality_data["score"] <= QUALITY_THRESHOLD:
            time.sleep(1)
            print("Cleaning low quality data")
            with self.output().open("w") as f:
                f.write("done")
        else:
            with self.output().open("w") as f:
                f.write("skipped")


class ProcessLowQuality(luigi.Task):
    def requires(self) -> CleanData:
        return CleanData()

    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("low_quality_processed.done")

    def run(self) -> None:
        with self.input().open("r") as f:  # type: ignore
            content: str = f.read()

        if content == "done":
            time.sleep(1)
            print("Processing cleaned low quality data")
            with self.output().open("w") as f:
                f.write("done")


class ProcessPartition(luigi.Task):
    partition = luigi.Parameter()

    def requires(self) -> list[luigi.Task]:
        return [ProcessHighQuality(), ProcessLowQuality()]

    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget(f"partition_{self.partition}.done")

    def run(self) -> None:
        time.sleep(1)
        print(f"Processing {self.partition}")
        with self.output().open("w") as f:
            f.write("done")


class CombineResults(luigi.Task):
    def requires(self) -> list[ProcessPartition]:
        partitions = ["partition_1", "partition_2", "partition_3"]
        return [ProcessPartition(partition=p) for p in partitions]

    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("combined_results.done")

    def run(self) -> None:
        time.sleep(1)
        print("Combined all partition results")
        with self.output().open("w") as f:
            f.write("done")


if __name__ == "__main__":
    luigi.build([CombineResults()], local_scheduler=True)
