"""Python tasks for the data_pipeline benchmark."""
import time
from typing import TypedDict


class ExtractOutput(TypedDict):
    records: list[int]
    source: str


class TransformOutput(TypedDict):
    records: list[int]
    source: str
    transformed: bool


def extract() -> ExtractOutput:
    time.sleep(1)
    data: ExtractOutput = {"records": [1, 2, 3, 4, 5], "source": "api"}
    print(f"Extracted {len(data['records'])} records")
    return data


def transform(data: ExtractOutput) -> TransformOutput:
    time.sleep(1)
    transformed = [x * 2 for x in data["records"]]
    print(f"Transformed {len(transformed)} records")
    return {"records": transformed, "source": data["source"], "transformed": True}


def load(data: TransformOutput) -> None:
    time.sleep(1)
    print(f"Loaded {len(data['records'])} records from {data['source']}")
    print(f"Final data: {data['records']}")
