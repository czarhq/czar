import time
from typing import Any

from mage_ai.data_preparation.decorators import (
    data_exporter,
    data_loader,
    transformer,
)


@data_loader
def extract(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
    time.sleep(1)
    data = {"records": [1, 2, 3, 4, 5], "source": "api"}
    print(f"Extracted {len(data['records'])} records")
    return data


@transformer
def transform(data: dict[str, Any], *_args: Any, **_kwargs: Any) -> dict[str, Any]:
    time.sleep(1)
    records = data["records"]
    transformed = [x * 2 for x in records]
    result = {"records": transformed, "source": data["source"], "transformed": True}
    print(f"Transformed {len(transformed)} records")
    return result


@data_exporter
def load(data: dict[str, Any], *_args: Any, **_kwargs: Any) -> None:
    time.sleep(1)
    print(f"Loaded {len(data['records'])} records from {data['source']}")
    print(f"Final data: {data['records']}")
