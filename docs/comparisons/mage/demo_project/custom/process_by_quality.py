import time
from typing import Any

if 'custom' not in globals():
    from mage_ai.data_preparation.decorators import custom  # noqa: F811

@custom
def process_data(quality_data: dict[str, Any], *args: Any, **kwargs: Any) -> dict[str, Any]:
    time.sleep(1)

    if quality_data["is_high_quality"]:
        print("Processing high quality data")
        result = {"path": "high_quality", "processed": True}
    else:
        print("Cleaning low quality data")
        time.sleep(1)
        print("Processing cleaned low quality data")
        result = {"path": "low_quality", "processed": True, "cleaned": True}

    return result
