import time
from typing import Any

from kedro.pipeline import Pipeline, node


def extract() -> dict[str, Any]:
    time.sleep(1)
    data = {"records": [1, 2, 3, 4, 5], "source": "api"}
    print(f"Extracted {len(data['records'])} records")
    return data


def transform(data: dict[str, Any]) -> dict[str, Any]:
    time.sleep(1)
    records: list[int] = data["records"]
    transformed = [x * 2 for x in records]
    result = {"records": transformed, "source": data["source"], "transformed": True}
    print(f"Transformed {len(transformed)} records")
    return result


def load(data: dict[str, Any]) -> None:
    time.sleep(1)
    print(f"Loaded {len(data['records'])} records from {data['source']}")
    print(f"Final data: {data['records']}")


def create_pipeline(**_kwargs: Any) -> Pipeline:
    return Pipeline(
        [
            node(func=extract, inputs=None, outputs="raw_data", name="extract"),
            node(
                func=transform,
                inputs="raw_data",
                outputs="clean_data",
                name="transform",
            ),
            node(func=load, inputs="clean_data", outputs=None, name="load"),
        ]
    )


if __name__ == "__main__":
    from kedro.io import DataCatalog
    from kedro.runner import SequentialRunner

    pipeline = create_pipeline()
    catalog = DataCatalog()
    runner = SequentialRunner()
    runner.run(pipeline, catalog=catalog)
