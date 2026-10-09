import time

from prefect import flow, task


@flow
def parallel_tasks_flow() -> None:
    fetch_users()
    fetch_orders()
    fetch_products()
    combine_data()


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


if __name__ == "__main__":
    parallel_tasks_flow()
