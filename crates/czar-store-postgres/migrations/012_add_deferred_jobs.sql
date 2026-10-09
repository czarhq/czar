CREATE TABLE IF NOT EXISTS deferred_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    service_type TEXT NOT NULL,
    job_id TEXT NOT NULL,
    job_data JSONB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'polling', 'completed', 'failed', 'cancelled')),
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    last_polled_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_deferred_jobs_status ON deferred_jobs(status);
CREATE INDEX IF NOT EXISTS idx_deferred_jobs_task_id ON deferred_jobs(task_id);
CREATE INDEX IF NOT EXISTS idx_deferred_jobs_created_at ON deferred_jobs(created_at);
