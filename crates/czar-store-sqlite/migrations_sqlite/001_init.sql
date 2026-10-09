-- SQLite schema for Czar (standalone mode)

PRAGMA foreign_keys = ON;

CREATE TABLE workflows (
    id BLOB PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    job_name TEXT NOT NULL,
    region TEXT NOT NULL DEFAULT 'us-central1',
    project TEXT NOT NULL,
    task_params TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    executor_type TEXT NOT NULL DEFAULT 'cloudrun'
);

CREATE TABLE runs (
    id BLOB PRIMARY KEY,
    workflow_id BLOB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'success', 'failed', 'cancelled')),
    triggered_by TEXT NOT NULL DEFAULT 'manual',
    started_at TEXT,
    finished_at TEXT,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id)
);

CREATE TABLE tasks (
    id BLOB PRIMARY KEY,
    run_id BLOB NOT NULL,
    task_index INTEGER NOT NULL,
    task_name TEXT NOT NULL,
    executor_type TEXT NOT NULL,
    depends_on TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL CHECK (status IN ('pending', 'dispatched', 'running', 'success', 'failed', 'cancelled')),
    execution_name TEXT,
    params TEXT,
    output TEXT,
    error TEXT,
    dispatched_at TEXT,
    started_at TEXT,
    finished_at TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(run_id, task_index),
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE TABLE workflow_tasks (
    id BLOB PRIMARY KEY,
    workflow_id BLOB NOT NULL,
    task_index INTEGER NOT NULL,
    task_name TEXT NOT NULL,
    executor_type TEXT NOT NULL,
    depends_on TEXT NOT NULL DEFAULT '[]',
    params TEXT,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(workflow_id, task_name),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

CREATE INDEX idx_runs_status ON runs(status);
CREATE INDEX idx_runs_workflow_id ON runs(workflow_id);
CREATE INDEX idx_tasks_run_id ON tasks(run_id);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_executor_type ON tasks(executor_type);
CREATE INDEX idx_tasks_run_status ON tasks(run_id, status);
CREATE INDEX idx_tasks_run_task_name ON tasks(run_id, task_name);
CREATE INDEX idx_workflow_tasks_workflow_id ON workflow_tasks(workflow_id);
CREATE INDEX idx_workflows_executor_type ON workflows(executor_type);
