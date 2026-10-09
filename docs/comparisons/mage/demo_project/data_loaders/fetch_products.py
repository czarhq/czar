import time
from typing import Any

if 'data_loader' not in globals():
    from mage_ai.data_preparation.decorators import data_loader


@data_loader
def load_data(*args: Any, **kwargs: Any) -> dict[str, str]:
    time.sleep(1)
    print("Fetched products")
    return {"status": "products_done"}
