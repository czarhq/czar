import time
from typing import Any

from metaflow.decorators import step
from metaflow.flowspec import FlowSpec


class DataPipelineFlow(FlowSpec):
    data: dict[str, Any]

    @step
    def start(self) -> None:
        self.next(self.extract)

    @step
    def extract(self) -> None:
        time.sleep(1)
        self.data = {"records": [1, 2, 3, 4, 5], "source": "api"}
        print(f"Extracted {len(self.data['records'])} records")
        self.next(self.transform)

    @step
    def transform(self) -> None:
        time.sleep(1)
        records: list[int] = self.data["records"]
        transformed = [x * 2 for x in records]
        self.data = {
            "records": transformed,
            "source": self.data["source"],
            "transformed": True,
        }
        print(f"Transformed {len(transformed)} records")
        self.next(self.load)

    @step
    def load(self) -> None:
        time.sleep(1)
        print(f"Loaded {len(self.data['records'])} records from {self.data['source']}")
        print(f"Final data: {self.data['records']}")
        self.next(self.end)

    @step
    def end(self) -> None:
        print("Workflow complete")


if __name__ == "__main__":
    DataPipelineFlow()
