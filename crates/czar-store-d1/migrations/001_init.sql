CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    job_name TEXT NOT NULL,
    region TEXT NOT NULL DEFAULT 'us-central1',
    project TEXT NOT NULL,
    executor_type TEXT NOT NULL DEFAULT 'cloudrun',
    task_params TEXT,
    schedule TEXT,
    schedule_enabled INTEGER NOT NULL DEFAULT 0,
    last_scheduled_at TEXT,
    next_scheduled_at TEXT,
    current_snapshot_id TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    snapshot_id TEXT,
    task_selection TEXT,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'paused', 'success', 'failed', 'cancelled')),
    triggered_by TEXT NOT NULL DEFAULT 'manual',
    started_at TEXT,
    finished_at TEXT,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    task_index INTEGER NOT NULL,
    task_name TEXT NOT NULL,
    executor_type TEXT NOT NULL,
    depends_on TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL CHECK (status IN ('pending', 'paused', 'dispatched', 'running', 'success', 'failed', 'cancelled')),
    attempts INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 0,
    timeout_seconds INTEGER,
    retry_at TEXT,
    execution_name TEXT,
    params TEXT,
    output TEXT,
    logs TEXT,
    error TEXT,
    progress TEXT,
    dispatched_at TEXT,
    started_at TEXT,
    finished_at TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(run_id, task_index),
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS workflow_tasks (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    task_index INTEGER NOT NULL,
    task_name TEXT NOT NULL,
    executor_type TEXT NOT NULL,
    depends_on TEXT NOT NULL DEFAULT '[]',
    params TEXT,
    signature TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(workflow_id, task_name),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS workflow_snapshots (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    tasks_json TEXT NOT NULL,
    source_yaml TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(workflow_id, content_hash),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS workflow_task_states (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    task_name TEXT NOT NULL,
    last_status TEXT NOT NULL CHECK (last_status IN ('pending', 'paused', 'dispatched', 'running', 'success', 'failed', 'cancelled')),
    last_run_id TEXT NOT NULL,
    last_task_id TEXT NOT NULL,
    last_output TEXT,
    last_error TEXT,
    last_finished_at TEXT,
    last_success_at TEXT,
    last_failure_at TEXT,
    updated_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(workflow_id, task_name),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS deferred_jobs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    service_type TEXT NOT NULL,
    job_id TEXT NOT NULL,
    job_data TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'polling', 'completed', 'failed', 'cancelled')),
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    started_at TEXT,
    last_polled_at TEXT,
    finished_at TEXT,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
CREATE INDEX IF NOT EXISTS idx_runs_workflow_id ON runs(workflow_id);
CREATE INDEX IF NOT EXISTS idx_tasks_run_id ON tasks(run_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_executor_type ON tasks(executor_type);
CREATE INDEX IF NOT EXISTS idx_tasks_run_status ON tasks(run_id, status);
CREATE INDEX IF NOT EXISTS idx_tasks_run_task_name ON tasks(run_id, task_name);
CREATE INDEX IF NOT EXISTS idx_workflow_tasks_workflow_id ON workflow_tasks(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflows_executor_type ON workflows(executor_type);
CREATE INDEX IF NOT EXISTS idx_workflows_next_scheduled ON workflows(next_scheduled_at) WHERE schedule_enabled = 1;
CREATE INDEX IF NOT EXISTS idx_workflow_snapshots_workflow_id ON workflow_snapshots(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflow_task_states_workflow_id ON workflow_task_states(workflow_id);
CREATE INDEX IF NOT EXISTS idx_deferred_jobs_status ON deferred_jobs(status);
CREATE INDEX IF NOT EXISTS idx_deferred_jobs_task_id ON deferred_jobs(task_id);
CREATE INDEX IF NOT EXISTS idx_deferred_jobs_created_at ON deferred_jobs(created_at);
