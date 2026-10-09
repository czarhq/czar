import time
from typing import Any

if 'data_loader' not in globals():
    from mage_ai.data_preparation.decorators import data_loader

HIGH_QUALITY_THRESHOLD = 0.8


@data_loader
def load_data(*args: Any, **kwargs: Any) -> dict[str, Any]:
    time.sleep(1)
    score = 0.6
    print(f"Data quality score: {score}")
    return {"score": score, "is_high_quality": score > HIGH_QUALITY_THRESHOLD}
