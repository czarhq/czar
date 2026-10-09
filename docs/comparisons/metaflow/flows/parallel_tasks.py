import time
from typing import Any

from metaflow import card
from metaflow.decorators import step
from metaflow.flowspec import FlowSpec


class ParallelTasksFlow(FlowSpec):
    @card
    @step
    def start(self) -> None:
        self.next(
            self.fetch_users, self.fetch_orders, self.fetch_products
        )

    @step
    def fetch_users(self) -> None:
        time.sleep(1)
        print("Fetched users")
        self.next(self.combine_data)

    @step
    def fetch_orders(self) -> None:
        time.sleep(1)
        print("Fetched orders")
        self.next(self.combine_data)

    @step
    def fetch_products(self) -> None:
        time.sleep(1)
        print("Fetched products")
        self.next(self.combine_data)

    @step
    def combine_data(self, inputs: Any) -> None:
        _ = inputs
        time.sleep(1)
        print("Combined all data")
        self.next(self.end)

    @card
    @step
    def end(self) -> None:
        print("Workflow complete")


if __name__ == "__main__":
    ParallelTasksFlow()
