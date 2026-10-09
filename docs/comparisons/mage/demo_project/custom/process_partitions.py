import time
from typing import Any

if 'custom' not in globals():
    from mage_ai.data_preparation.decorators import custom  # noqa: F811


@custom
def process_data(
    processing_result: dict[str, Any],
    partitions: list[str],
    *args: Any,
    **kwargs: Any,
) -> list[dict[str, str]]:
    results: list[dict[str, str]] = []
    for partition in partitions:
        time.sleep(1)
        print(f"Processing {partition}")
        results.append({"partition": partition, "status": "processed"})

    return results
