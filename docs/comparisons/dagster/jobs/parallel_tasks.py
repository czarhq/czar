import time

from dagster import asset


@asset
def fetch_users() -> None:
    time.sleep(1)
    print("Fetched users")


@asset
def fetch_orders() -> None:
    time.sleep(1)
    print("Fetched orders")


@asset
def fetch_products() -> None:
    time.sleep(1)
    print("Fetched products")


@asset(deps=[fetch_users, fetch_orders, fetch_products])
def combine_data() -> None:
    time.sleep(1)
    print("Combined all data")
