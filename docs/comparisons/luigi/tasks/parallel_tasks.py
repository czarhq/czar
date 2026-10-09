import time

import luigi


class FetchUsers(luigi.Task):
    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("users.done")

    def run(self) -> None:
        time.sleep(1)
        print("Fetched users")
        with self.output().open("w") as f:
            f.write("done")


class FetchOrders(luigi.Task):
    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("orders.done")

    def run(self) -> None:
        time.sleep(1)
        print("Fetched orders")
        with self.output().open("w") as f:
            f.write("done")


class FetchProducts(luigi.Task):
    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("products.done")

    def run(self) -> None:
        time.sleep(1)
        print("Fetched products")
        with self.output().open("w") as f:
            f.write("done")


class CombineData(luigi.Task):
    def requires(self) -> list[luigi.Task]:
        return [FetchUsers(), FetchOrders(), FetchProducts()]

    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("combined.done")

    def run(self) -> None:
        time.sleep(1)
        print("Combined all data")
        with self.output().open("w") as f:
            f.write("done")


if __name__ == "__main__":
    luigi.build([CombineData()], local_scheduler=True)
