//! Google Cloud job trackers.

mod bigquery;
mod cloudrun;
mod dataproc;

use std::sync::Arc;

use czar_core::job_tracker::JobTracker;

pub use bigquery::BigQueryTracker;
#[cfg(test)]
pub(crate) use bigquery::{BigQueryJobStatus, map_bigquery_status};
pub use cloudrun::CloudRunTracker;
#[cfg(test)]
pub(crate) use cloudrun::{CloudRunExecutionResponse, map_cloudrun_execution};
pub use dataproc::DataprocTracker;
#[cfg(test)]
pub(crate) use dataproc::{DataprocJobResponse, map_dataproc_job};

pub async fn default_trackers() -> anyhow::Result<Vec<Arc<dyn JobTracker>>> {
    Ok(vec![
        Arc::new(BigQueryTracker::new().await?),
        Arc::new(CloudRunTracker::new().await?),
        Arc::new(DataprocTracker::new().await?),
    ])
}

#[cfg(test)]
mod tests;
