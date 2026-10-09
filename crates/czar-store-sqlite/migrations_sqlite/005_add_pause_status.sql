CREATE TABLE runs_new (
    id BLOB PRIMARY KEY,
    workflow_id BLOB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'paused', 'success', 'failed', 'cancelled')),
    triggered_by TEXT NOT NULL DEFAULT 'manual',
    started_at TEXT,
    finished_at TEXT,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id)
);

INSERT INTO runs_new (id, workflow_id, status, triggered_by, started_at, finished_at, error, created_at)
SELECT id, workflow_id, status, triggered_by, started_at, finished_at, error, created_at FROM runs;

CREATE TABLE tasks_tmp (
    id BLOB PRIMARY KEY,
    run_id BLOB NOT NULL,
    task_index INTEGER NOT NULL,
    task_name TEXT NOT NULL,
    executor_type TEXT NOT NULL,
    depends_on TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL CHECK (status IN ('pending', 'paused', 'dispatched', 'running', 'success', 'failed', 'cancelled')),
    execution_name TEXT,
    params TEXT,
    output TEXT,
    error TEXT,
    dispatched_at TEXT,
    started_at TEXT,
    finished_at TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    logs TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 0,
    timeout_seconds INTEGER,
    retry_at TEXT,
    UNIQUE(run_id, task_index)
);

INSERT INTO tasks_tmp (
    id,
    run_id,
    task_index,
    task_name,
    executor_type,
    depends_on,
    status,
    execution_name,
    params,
    output,
    error,
    dispatched_at,
    started_at,
    finished_at,
    created_at,
    logs,
    attempts,
    max_retries,
    timeout_seconds,
    retry_at
)
SELECT
    id,
    run_id,
    task_index,
    task_name,
    executor_type,
    depends_on,
    status,
    execution_name,
    params,
    output,
    error,
    dispatched_at,
    started_at,
    finished_at,
    created_at,
    logs,
    attempts,
    max_retries,
    timeout_seconds,
    retry_at
FROM tasks;

DROP TABLE tasks;
DROP TABLE runs;
ALTER TABLE runs_new RENAME TO runs;

CREATE TABLE tasks_new (
    id BLOB PRIMARY KEY,
    run_id BLOB NOT NULL,
    task_index INTEGER NOT NULL,
    task_name TEXT NOT NULL,
    executor_type TEXT NOT NULL,
    depends_on TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL CHECK (status IN ('pending', 'paused', 'dispatched', 'running', 'success', 'failed', 'cancelled')),
    execution_name TEXT,
    params TEXT,
    output TEXT,
    error TEXT,
    dispatched_at TEXT,
    started_at TEXT,
    finished_at TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    logs TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 0,
    timeout_seconds INTEGER,
    retry_at TEXT,
    UNIQUE(run_id, task_index),
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
);

INSERT INTO tasks_new (
    id,
    run_id,
    task_index,
    task_name,
    executor_type,
    depends_on,
    status,
    execution_name,
    params,
    output,
    error,
    dispatched_at,
    started_at,
    finished_at,
    created_at,
    logs,
    attempts,
    max_retries,
    timeout_seconds,
    retry_at
)
SELECT
    id,
    run_id,
    task_index,
    task_name,
    executor_type,
    depends_on,
    status,
    execution_name,
    params,
    output,
    error,
    dispatched_at,
    started_at,
    finished_at,
    created_at,
    logs,
    attempts,
    max_retries,
    timeout_seconds,
    retry_at
FROM tasks_tmp;

DROP TABLE tasks_tmp;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE INDEX idx_runs_status ON runs(status);
CREATE INDEX idx_runs_workflow_id ON runs(workflow_id);

CREATE INDEX idx_tasks_run_id ON tasks(run_id);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_executor_type ON tasks(executor_type);
CREATE INDEX idx_tasks_run_status ON tasks(run_id, status);
CREATE INDEX idx_tasks_run_task_name ON tasks(run_id, task_name);
