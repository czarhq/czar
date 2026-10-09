"""Project pipelines."""

from typing import Any

from kedro.pipeline import Pipeline

import sys
sys.path.insert(0, "pipelines")

from parallel_tasks import create_pipeline as create_parallel_tasks
from data_pipeline import create_pipeline as create_data_pipeline
from conditional_branching import create_pipeline as create_conditional_branching


def register_pipelines() -> dict[str, Pipeline]:
    """Register the project's pipelines."""
    return {
        "__default__": create_parallel_tasks() + create_data_pipeline() + create_conditional_branching(),
        "parallel_tasks": create_parallel_tasks(),
        "data_pipeline": create_data_pipeline(),
        "conditional_branching": create_conditional_branching(),
    }
