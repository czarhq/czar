import time
from typing import Any

if 'transformer' not in globals():
    from mage_ai.data_preparation.decorators import transformer


@transformer
def transform(data: dict[str, Any], *args: Any, **kwargs: Any) -> dict[str, Any]:
    time.sleep(1)
    records = data["records"]
    transformed = [x * 2 for x in records]
    result = {"records": transformed, "source": data["source"], "transformed": True}
    print(f"Transformed {len(transformed)} records")
    return result
