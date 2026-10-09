import time
from typing import Any

from mage_ai.data_preparation.decorators import (
    data_exporter,
    data_loader,
)


@data_loader
def fetch_users(*_args: Any, **_kwargs: Any) -> dict[str, str]:
    time.sleep(1)
    print("Fetched users")
    return {"status": "users_done"}


@data_loader
def fetch_orders(*_args: Any, **_kwargs: Any) -> dict[str, str]:
    time.sleep(1)
    print("Fetched orders")
    return {"status": "orders_done"}


@data_loader
def fetch_products(*_args: Any, **_kwargs: Any) -> dict[str, str]:
    time.sleep(1)
    print("Fetched products")
    return {"status": "products_done"}


@data_exporter
def combine_data(
    users: dict[str, str],
    orders: dict[str, str],
    products: dict[str, str],
    *_args: Any,
    **_kwargs: Any,
) -> None:
    time.sleep(1)
    print("Combined all data")
    print(f"Status: {users['status']}, {orders['status']}, {products['status']}")
