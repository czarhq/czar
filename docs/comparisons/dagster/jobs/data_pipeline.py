import time
from typing import Any

from dagster import asset


@asset
def extract() -> dict[str, Any]:
    time.sleep(1)
    data = {"records": [1, 2, 3, 4, 5], "source": "api"}
    print(f"Extracted {len(data['records'])} records")
    return data


@asset
def transform(extract: dict[str, Any]) -> dict[str, Any]:
    time.sleep(1)
    records = extract["records"]
    transformed = [x * 2 for x in records]
    result = {"records": transformed, "source": extract["source"], "transformed": True}
    print(f"Transformed {len(transformed)} records")
    return result


@asset
def load(transform: dict[str, Any]) -> None:
    time.sleep(1)
    print(f"Loaded {len(transform['records'])} records from {transform['source']}")
    print(f"Final data: {transform['records']}")
