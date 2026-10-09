"""
Example workflow demonstrating deferrables with Czar.

This example shows how to use deferrables to track long-running external jobs
without keeping the worker container running.
"""

import os
from typing import Any, TypedDict

from czar import CustomHttp


class DeferredOutput(TypedDict):
    deferred: list[dict[str, Any]]


class ProcessResultsOutput(TypedDict):
    message: str
    upstream_tasks: list[str]


def simulate_bigquery_job() -> DeferredOutput:
    """
    Simulates starting a BigQuery job and returning a deferrable.

    In a real scenario, this would:
    1. Use google-cloud-bigquery to start a query
    2. Get the job_id from the BigQuery API
    3. Return BigQueryJob deferrable

    The worker then scales to 0, and the scheduler tracks the job.
    """
    job_id = f"simulated_job_{os.getpid()}"

    print(f"Starting mock BigQuery job: {job_id}")
    print("Tracker URL: mock://success")

    return {
        "deferred": [
            CustomHttp(
                url=f"mock://success/{job_id}",
                method="GET",
                completion_field="status",
                completion_value="success",
                failure_value="failed",
            ).to_dict()
        ]
    }


def simulate_custom_http_job() -> DeferredOutput:
    """
    Simulates a custom HTTP job that polls a status endpoint.

    This is useful for tracking jobs from custom APIs that provide
    status endpoints.
    """
    job_id = f"custom_job_{os.getpid()}"
    status_url = f"mock://success/{job_id}"

    print(f"Starting custom HTTP job: {job_id}")
    print(f"Status URL: {status_url}")

    return {
        "deferred": [
            CustomHttp(
                url=status_url,
                method="GET",
                headers={"Authorization": "Bearer fake-token"},
                completion_field="status",
                completion_value="completed",
                failure_value="failed",
            ).to_dict()
        ]
    }


def multiple_jobs() -> DeferredOutput:
    """
    Example of returning multiple deferrables.

    The task completes only when all deferred jobs complete.
    """
    print("Starting multiple mock deferred jobs")

    return {
        "deferred": [
            CustomHttp(
                url=f"mock://success/analytics_job_{os.getpid()}",
                method="GET",
                completion_field="status",
                completion_value="success",
                failure_value="failed",
            ).to_dict(),
            CustomHttp(
                url=f"mock://success/ml_training_{os.getpid()}",
                method="GET",
                completion_field="status",
                completion_value="success",
                failure_value="failed",
            ).to_dict(),
        ]
    }


def process_results() -> ProcessResultsOutput:
    """
    Example of a downstream task that runs after deferred jobs complete.

    This task would fetch and process the results from the BigQuery tables
    that were populated by the previous task's deferred job.
    """
    print("Processing results from 1 upstream task")

    return {
        "message": "Results processed successfully",
        "upstream_tasks": ["bigquery_job"],
    }
