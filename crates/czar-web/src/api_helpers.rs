use chrono::{DateTime, Utc};

pub fn fmt_time(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339()
}

pub fn is_not_found(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("row not found") || msg.contains("no rows") || msg.contains("not found")
}
