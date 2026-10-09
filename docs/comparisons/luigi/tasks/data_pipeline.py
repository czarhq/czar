import json
import time
from typing import Any

import luigi


class Extract(luigi.Task):
    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("extracted.json")

    def run(self) -> None:
        time.sleep(1)
        data = {"records": [1, 2, 3, 4, 5], "source": "api"}
        print(f"Extracted {len(data['records'])} records")
        with self.output().open("w") as f:
            json.dump(data, f)


class Transform(luigi.Task):
    def requires(self) -> Extract:
        return Extract()

    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("transformed.json")

    def run(self) -> None:
        time.sleep(1)
        with self.input().open("r") as f: # type: ignore
            data: dict[str, Any] = json.load(f)

        records: list[int] = data["records"]
        transformed = [x * 2 for x in records]
        result = {"records": transformed, "source": data["source"], "transformed": True}
        print(f"Transformed {len(transformed)} records")

        with self.output().open("w") as f:
            json.dump(result, f)


class Load(luigi.Task):
    def requires(self) -> Transform:
        return Transform()

    def output(self) -> luigi.LocalTarget:  # type: ignore
        return luigi.LocalTarget("loaded.done")

    def run(self) -> None:
        time.sleep(1)
        with self.input().open("r") as f: # type: ignore
            data: dict[str, Any] = json.load(f)

        print(f"Loaded {len(data['records'])} records from {data['source']}")
        print(f"Final data: {data['records']}")

        with self.output().open("w") as f:
            f.write("done")


if __name__ == "__main__":
    luigi.build([Load()], local_scheduler=True)
