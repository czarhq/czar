# czar-gcp

_Updated at: 2026-05-22_

Google Cloud integrations for Czar. This crate owns GCP-specific auth and API calls so `czar-core` can stay provider-neutral.

## Owns

- BigQuery deferred-job tracker
- Cloud Run execution tracker
- Dataproc job tracker
- GCP tracker registration helpers for hosts such as `czar-cli`

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/czar-gcp/Cargo.toml) | GCP crate manifest and explicit GCP/auth dependencies. | 2026-05-17 | pending |
| [src/job_tracker/bigquery.rs](../../../crates/czar-gcp/src/job_tracker/bigquery.rs) | BigQuery deferred-job tracker with REST polling and job-state mapping. | 2026-05-17 | pending |
| [src/job_tracker/cloudrun.rs](../../../crates/czar-gcp/src/job_tracker/cloudrun.rs) | Cloud Run deferred-job tracker with REST polling and execution-status mapping. | 2026-05-17 | pending |
| [src/job_tracker/dataproc.rs](../../../crates/czar-gcp/src/job_tracker/dataproc.rs) | Dataproc deferred-job tracker with REST polling and status translation. | 2026-05-17 | pending |
| [src/job_tracker/mod.rs](../../../crates/czar-gcp/src/job_tracker/mod.rs) | GCP tracker module wiring and default tracker construction. | 2026-05-17 | pending |
