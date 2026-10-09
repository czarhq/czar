import time
from typing import Any

from prefect import flow, task


@flow
def data_pipeline_flow() -> None:
    raw_data = extract()
    clean_data = transform(raw_data)
    load(clean_data)


@task
def extract() -> dict[str, Any]:
    time.sleep(1)
    data = {"records": [1, 2, 3, 4, 5], "source": "api"}
    print(f"Extracted {len(data['records'])} records")
    return data


@task
def transform(data: dict[str, Any]) -> dict[str, Any]:
    time.sleep(1)
    records = data["records"]
    transformed = [x * 2 for x in records]
    result = {"records": transformed, "source": data["source"], "transformed": True}
    print(f"Transformed {len(transformed)} records")
    return result


@task
def load(data: dict[str, Any]) -> None:
    time.sleep(1)
    print(f"Loaded {len(data['records'])} records from {data['source']}")
    print(f"Final data: {data['records']}")


if __name__ == "__main__":
    data_pipeline_flow()
