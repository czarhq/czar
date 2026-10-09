CREATE TABLE IF NOT EXISTS deferred_jobs (
    id BLOB PRIMARY KEY DEFAULT (randomblob(16)),
    task_id BLOB NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    service_type TEXT NOT NULL,
    job_id TEXT NOT NULL,
    job_data TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'polling', 'completed', 'failed', 'cancelled')),
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    started_at TEXT,
    last_polled_at TEXT,
    finished_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_deferred_jobs_status ON deferred_jobs(status);
CREATE INDEX IF NOT EXISTS idx_deferred_jobs_task_id ON deferred_jobs(task_id);
CREATE INDEX IF NOT EXISTS idx_deferred_jobs_created_at ON deferred_jobs(created_at);
