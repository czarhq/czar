from typing import Any

if 'data_loader' not in globals():
    from mage_ai.data_preparation.decorators import data_loader


@data_loader
def load_data(*args: Any, **kwargs: Any) -> list[str]:
    return ["partition_1", "partition_2", "partition_3"]
