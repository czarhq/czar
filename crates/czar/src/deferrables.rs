//! Deferrable types for long-running external jobs
//!
//! Deferrables allow tasks to start long-running external jobs (BigQuery, Cloud Run, etc.)
//! and return immediately. The Czar scheduler tracks these jobs via the Triggerer component,
//! allowing workers to scale to zero while jobs run.
//!
//! # Example
//!
//! ```rust,ignore
//! use czar_rust::deferrables::BigQueryJob;
//!
//! fn my_task(input: MyInput) -> BigQueryJob {
//!     // Start BigQuery job
//!     let job = start_bigquery_job();
//!
//!     // Return deferrable - worker can scale to 0
//!     BigQueryJob {
//!         project: "my-project".to_string(),
//!         job_id: job.id,
//!         location: "US".to_string(),
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trait for deferrable job types
///
/// Deferrables represent long-running external jobs that the scheduler
/// should track. Implementing this trait allows the scheduler to:
/// 1. Detect that a task returned a deferrable
/// 2. Extract job tracking information
/// 3. Route to the appropriate JobTracker implementation
pub trait Deferrable: Send + Sync {
    /// Get the service type for routing to the correct JobTracker
    fn service_type(&self) -> &str;

    /// Get job identifier for tracking
    fn job_id(&self) -> String;

    /// Serialize deferrable to JSON for database storage
    fn to_json(&self) -> serde_json::Value;
}

/// BigQuery job deferrable
///
/// Used for long-running BigQuery queries. The scheduler will poll
/// the BigQuery API until the job completes.
///
/// # Example
///
/// ```rust,ignore
/// use google_cloud_bigquery::client::Client;
/// use czar_rust::deferrables::BigQueryJob;
///
/// async fn run_analytics() -> BigQueryJob {
///     let client = Client::new("my-project").await.unwrap();
///     let job = client.query("SELECT * FROM dataset.table").await.unwrap();
///
///     BigQueryJob {
///         project: "my-project".to_string(),
///         job_id: job.job_id().to_string(),
///         location: "US".to_string(),
///     }
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BigQueryJob {
    /// GCP project ID
    pub project: String,
    /// BigQuery job ID
    pub job_id: String,
    /// BigQuery location (e.g., "US", "EU")
    pub location: String,
}

impl Deferrable for BigQueryJob {
    fn service_type(&self) -> &str {
        "bigquery"
    }

    fn job_id(&self) -> String {
        format!("{}/{}/{}", self.project, self.location, self.job_id)
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Cloud Run job deferrable
///
/// Used for Cloud Run jobs. The scheduler will poll the Cloud Run API
/// until the job execution completes.
///
/// # Example
///
/// ```rust,ignore
/// use czar_rust::deferrables::CloudRunJob;
///
/// async fn trigger_batch_job() -> CloudRunJob {
///     let execution_id = start_cloud_run_job("my-job").await;
///
///     CloudRunJob {
///         project: "my-project".to_string(),
///         region: "us-central1".to_string(),
///         job_name: "my-job".to_string(),
///         execution_id,
///     }
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CloudRunJob {
    /// GCP project ID
    pub project: String,
    /// Cloud Run region
    pub region: String,
    /// Cloud Run job name
    pub job_name: String,
    /// Execution ID
    pub execution_id: String,
}

impl Deferrable for CloudRunJob {
    fn service_type(&self) -> &str {
        "cloudrun"
    }

    fn job_id(&self) -> String {
        format!(
            "{}/{}/{}/{}",
            self.project, self.region, self.job_name, self.execution_id
        )
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Dataproc job deferrable
///
/// Used for Dataproc Spark/Hadoop jobs. The scheduler will poll the
/// Dataproc API until the job completes.
///
/// # Example
///
/// ```rust,ignore
/// use czar_rust::deferrables::DataprocJob;
///
/// async fn run_spark_job() -> DataprocJob {
///     let job_id = submit_dataproc_job("my-cluster", "spark-job.py").await;
///
///     DataprocJob {
///         project: "my-project".to_string(),
///         region: "us-central1".to_string(),
///         cluster_name: "my-cluster".to_string(),
///         job_id,
///     }
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataprocJob {
    /// GCP project ID
    pub project: String,
    /// Dataproc region
    pub region: String,
    /// Cluster name
    pub cluster_name: String,
    /// Job ID
    pub job_id: String,
}

impl Deferrable for DataprocJob {
    fn service_type(&self) -> &str {
        "dataproc"
    }

    fn job_id(&self) -> String {
        format!(
            "{}/{}/{}/{}",
            self.project, self.region, self.cluster_name, self.job_id
        )
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Custom HTTP polling deferrable
///
/// Used for custom external services that provide HTTP status endpoints.
/// The scheduler will poll the URL until the response indicates completion.
///
/// # Example
///
/// ```rust,ignore
/// use czar_rust::deferrables::CustomHttp;
/// use std::collections::HashMap;
///
/// async fn trigger_external_job() -> CustomHttp {
///     let job_id = start_external_job().await;
///
///     let mut headers = HashMap::new();
///     headers.insert("Authorization".to_string(), "Bearer token".to_string());
///
///     CustomHttp {
///         url: format!("https://api.example.com/jobs/{}/status", job_id),
///         method: "GET".to_string(),
///         headers,
///         success_status_codes: vec![200],
///         completion_field: "status".to_string(),
///         completion_value: "completed".to_string(),
///         failure_value: Some("failed".to_string()),
///     }
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CustomHttp {
    /// URL to poll for job status
    pub url: String,
    /// HTTP method (GET, POST, etc.)
    pub method: String,
    /// Optional HTTP headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// HTTP status codes that indicate successful polling (default: [200])
    #[serde(default = "default_success_codes")]
    pub success_status_codes: Vec<u16>,
    /// JSON field to check for completion status
    pub completion_field: String,
    /// Value indicating job completion
    pub completion_value: String,
    /// Optional value indicating job failure
    pub failure_value: Option<String>,
}

fn default_success_codes() -> Vec<u16> {
    vec![200]
}

impl Deferrable for CustomHttp {
    fn service_type(&self) -> &str {
        "custom_http"
    }

    fn job_id(&self) -> String {
        // Use URL as job identifier
        self.url.clone()
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Deferrable wrapper for task outputs
///
/// Tasks can return either:
/// - Regular data (synchronous completion)
/// - One or more deferrables (asynchronous tracking)
/// - Both regular data and deferrables
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TaskResult<T> {
    /// Synchronous result with regular data
    Data(T),
    /// Asynchronous result with deferrables
    Deferred {
        /// List of deferrables to track
        deferred: Vec<serde_json::Value>,
    },
    /// Mixed result with both data and deferrables
    Mixed {
        /// Regular output data
        data: T,
        /// List of deferrables to track
        deferred: Vec<serde_json::Value>,
    },
}

impl<T> TaskResult<T> {
    /// Create a result with only data
    pub fn data(value: T) -> Self {
        TaskResult::Data(value)
    }

    /// Create a result with only deferrables
    pub fn deferred(deferrables: Vec<Box<dyn Deferrable>>) -> Self {
        let deferred = deferrables.iter().map(|d| d.to_json()).collect();
        TaskResult::Deferred { deferred }
    }

    /// Create a result with both data and deferrables
    pub fn mixed(data: T, deferrables: Vec<Box<dyn Deferrable>>) -> Self {
        let deferred = deferrables.iter().map(|d| d.to_json()).collect();
        TaskResult::Mixed { data, deferred }
    }
}

#[cfg(test)]
mod tests;
