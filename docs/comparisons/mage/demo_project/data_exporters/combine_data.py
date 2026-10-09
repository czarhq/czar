import time
from typing import Any

if 'data_exporter' not in globals():
    from mage_ai.data_preparation.decorators import data_exporter


@data_exporter
def export_data(
    users: dict[str, str],
    orders: dict[str, str],
    products: dict[str, str],
    *args: Any,
    **kwargs: Any,
) -> None:
    time.sleep(1)
    print("Combined all data")
    print(f"Status: {users['status']}, {orders['status']}, {products['status']}")
