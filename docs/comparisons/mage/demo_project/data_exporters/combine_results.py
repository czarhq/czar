import time
from typing import Any

if 'data_exporter' not in globals():
    from mage_ai.data_preparation.decorators import data_exporter  # noqa: F811


@data_exporter
def export_data(partition_results: list[dict[str, str]], *args: Any, **kwargs: Any) -> None:
    time.sleep(1)
    print(f"Combined {len(partition_results)} partition results")
    print(f"Results: {partition_results}")
