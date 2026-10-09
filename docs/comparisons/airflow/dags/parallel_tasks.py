import time
from datetime import UTC, datetime

from airflow.sdk import dag, task


@dag(
    dag_id="parallel_tasks_flow",
    start_date=datetime(2024, 1, 1, tzinfo=UTC),
    schedule=None,
    catchup=False,
)
def parallel_tasks_flow() -> None:
    users = fetch_users()
    orders = fetch_orders()
    products = fetch_products()
    combine = combine_data()
    [users, orders, products] >> combine  # type: ignore


@task
def fetch_users() -> None:
    time.sleep(1)
    print("Fetched users")


@task
def fetch_orders() -> None:
    time.sleep(1)
    print("Fetched orders")


@task
def fetch_products() -> None:
    time.sleep(1)
    print("Fetched products")


@task
def combine_data() -> None:
    time.sleep(1)
    print("Combined all data")


parallel_tasks_flow()
