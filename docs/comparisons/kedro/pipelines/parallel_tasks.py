import time
from typing import Any

from kedro.io import DataCatalog, MemoryDataset
from kedro.pipeline import Pipeline, node
from kedro.runner import SequentialRunner


def fetch_users() -> dict[str, str]:
    time.sleep(1)
    print("Fetched users")
    return {"status": "users_done"}


def fetch_orders() -> dict[str, str]:
    time.sleep(1)
    print("Fetched orders")
    return {"status": "orders_done"}


def fetch_products() -> dict[str, str]:
    time.sleep(1)
    print("Fetched products")
    return {"status": "products_done"}


def combine_data(
    users: dict[str, str], orders: dict[str, str], products: dict[str, str]
) -> None:
    time.sleep(1)
    print("Combined all data")
    print(f"Status: {users['status']}, {orders['status']}, {products['status']}")


def create_pipeline(**_kwargs: Any) -> Pipeline:
    return Pipeline(
        [
            node(func=fetch_users, inputs=None, outputs="users", name="fetch_users"),
            node(func=fetch_orders, inputs=None, outputs="orders", name="fetch_orders"),
            node(
                func=fetch_products, inputs=None, outputs="products", name="fetch_products"
            ),
            node(
                func=combine_data,
                inputs=["users", "orders", "products"],
                outputs=None,
                name="combine_data",
            ),
        ]
    )


if __name__ == "__main__":
    pipeline = create_pipeline()
    catalog = DataCatalog()
    runner = SequentialRunner()
    runner.run(pipeline, catalog=catalog)
