"""Deferrable types for long-running external jobs

Deferrables allow tasks to start long-running external jobs (BigQuery, Cloud Run, etc.)
and return immediately. The Czar scheduler tracks these jobs via the Triggerer component,
allowing workers to scale to zero while jobs run.

Example:
    from czar import BigQueryJob
    from google.cloud import bigquery

    def my_task(input_data):
        # Start BigQuery job
        client = bigquery.Client(project="my-project")
        job = client.query("SELECT * FROM dataset.table")

        # Return deferrable - worker can scale to 0
        return BigQueryJob(
            project="my-project",
            job_id=job.job_id,
            location="US"
        )
"""

from dataclasses import dataclass, field, asdict
from typing import Dict, List, Optional, Any, Union
from abc import ABC, abstractmethod
import json


class Deferrable(ABC):
    """Base class for deferrable job types

    Deferrables represent long-running external jobs that the scheduler
    should track. Implementing this class allows the scheduler to:
    1. Detect that a task returned a deferrable
    2. Extract job tracking information
    3. Route to the appropriate JobTracker implementation
    """

    @abstractmethod
    def service_type(self) -> str:
        """Get the service type for routing to the correct JobTracker"""
        pass

    @abstractmethod
    def tracking_id(self) -> str:
        """Get a stable identifier for tracking this deferred job"""
        pass

    def to_dict(self) -> Dict[str, Any]:
        """Serialize deferrable to dictionary for JSON storage"""
        payload = asdict(self)
        payload["service_type"] = self.service_type()
        payload.setdefault("job_id", self.tracking_id())
        return payload

    def to_json(self) -> str:
        """Serialize deferrable to JSON string"""
        return json.dumps(self.to_dict())


@dataclass
class BigQueryJob(Deferrable):
    """BigQuery job deferrable

    Used for long-running BigQuery queries. The scheduler will poll
    the BigQuery API until the job completes.

    Example:
        from google.cloud import bigquery
        from czar import BigQueryJob

        def run_analytics():
            client = bigquery.Client(project="my-project")
            job = client.query("SELECT * FROM dataset.table")

            return BigQueryJob(
                project="my-project",
                job_id=job.job_id,
                location="US"
            )

    Attributes:
        project: GCP project ID
        job_id: BigQuery job ID
        location: BigQuery location (e.g., "US", "EU")
    """

    project: str
    job_id: str
    location: str

    def service_type(self) -> str:
        return "bigquery"

    def tracking_id(self) -> str:
        return f"{self.project}/{self.location}/{self.job_id}"


@dataclass
class CloudRunJob(Deferrable):
    """Cloud Run job deferrable

    Used for Cloud Run jobs. The scheduler will poll the Cloud Run API
    until the job execution completes.

    Example:
        from czar import CloudRunJob

        def trigger_batch_job():
            execution_id = start_cloud_run_job("my-job")

            return CloudRunJob(
                project="my-project",
                region="us-central1",
                job_name="my-job",
                execution_id=execution_id
            )

    Attributes:
        project: GCP project ID
        region: Cloud Run region
        job_name: Cloud Run job name
        execution_id: Execution ID
    """

    project: str
    region: str
    job_name: str
    execution_id: str

    def service_type(self) -> str:
        return "cloudrun"

    def tracking_id(self) -> str:
        return f"{self.project}/{self.region}/{self.job_name}/{self.execution_id}"


@dataclass
class DataprocJob(Deferrable):
    """Dataproc job deferrable

    Used for Dataproc Spark/Hadoop jobs. The scheduler will poll the
    Dataproc API until the job completes.

    Example:
        from czar import DataprocJob

        def run_spark_job():
            job_id = submit_dataproc_job("my-cluster", "spark-job.py")

            return DataprocJob(
                project="my-project",
                region="us-central1",
                cluster_name="my-cluster",
                job_id=job_id
            )

    Attributes:
        project: GCP project ID
        region: Dataproc region
        cluster_name: Cluster name
        job_id: Job ID
    """

    project: str
    region: str
    cluster_name: str
    job_id: str

    def service_type(self) -> str:
        return "dataproc"

    def tracking_id(self) -> str:
        return f"{self.project}/{self.region}/{self.cluster_name}/{self.job_id}"


@dataclass
class CustomHttp(Deferrable):
    """Custom HTTP polling deferrable

    Used for custom external services that provide HTTP status endpoints.
    The scheduler will poll the URL until the response indicates completion.

    Example:
        from czar import CustomHttp

        def trigger_external_job():
            job_id = start_external_job()

            return CustomHttp(
                url=f"https://api.example.com/jobs/{job_id}/status",
                method="GET",
                headers={"Authorization": "Bearer token"},
                success_status_codes=[200],
                completion_field="status",
                completion_value="completed",
                failure_value="failed"
            )

    Attributes:
        url: URL to poll for job status
        method: HTTP method (GET, POST, etc.)
        headers: Optional HTTP headers
        success_status_codes: HTTP status codes that indicate successful polling
        completion_field: JSON field to check for completion status
        completion_value: Value indicating job completion
        failure_value: Optional value indicating job failure
    """

    url: str
    method: str = "GET"
    headers: Dict[str, str] = field(default_factory=dict)
    success_status_codes: List[int] = field(default_factory=lambda: [200])
    completion_field: str = "status"
    completion_value: str = "completed"
    failure_value: Optional[str] = None

    def service_type(self) -> str:
        return "custom_http"

    def to_dict(self) -> Dict[str, Any]:
        payload = super().to_dict()
        payload["status_field"] = payload.pop("completion_field")
        payload["success_value"] = payload.pop("completion_value")
        return payload

    def tracking_id(self) -> str:
        # Use URL as job identifier
        return self.url


# Type alias for task results
TaskResult = Union[Any, Deferrable, List[Deferrable]]
