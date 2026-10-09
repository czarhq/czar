"""Python tasks for the parallel_tasks benchmark."""
import time


def fetch_users() -> None:
    time.sleep(1)
    print("Fetched users")


def fetch_orders() -> None:
    time.sleep(1)
    print("Fetched orders")


def fetch_products() -> None:
    time.sleep(1)
    print("Fetched products")


def combine_data() -> None:
    time.sleep(1)
    print("Combined all data")
