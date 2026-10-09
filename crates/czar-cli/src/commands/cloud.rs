use std::sync::Arc;

use anyhow::Result;
use czar_core::job_tracker::JobTracker;

pub async fn job_trackers() -> Result<Vec<Arc<dyn JobTracker>>> {
    #[cfg(feature = "gcp")]
    let trackers = czar_gcp::job_tracker::default_trackers().await;

    #[cfg(not(feature = "gcp"))]
    let trackers = Ok(Vec::new());

    trackers
}
