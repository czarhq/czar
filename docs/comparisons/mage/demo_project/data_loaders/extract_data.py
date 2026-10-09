import time
from typing import Any

if 'data_loader' not in globals():
    from mage_ai.data_preparation.decorators import data_loader


@data_loader
def load_data(*args: Any, **kwargs: Any) -> dict[str, Any]:
    time.sleep(1)
    data = {"records": [1, 2, 3, 4, 5], "source": "api"}
    print(f"Extracted {len(data['records'])} records")
    return data
