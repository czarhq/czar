-- Add 'skipped' to the tasks status CHECK constraint.
-- SQLite does not support ALTER CONSTRAINT, so we recreate the table.

CREATE TABLE tasks_new (
    id BLOB PRIMARY KEY,
    run_id BLOB NOT NULL,
    task_index INTEGER NOT NULL,
    task_name TEXT NOT NULL,
    executor_type TEXT NOT NULL,
    depends_on TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL CHECK (status IN ('pending', 'paused', 'dispatched', 'running', 'success', 'failed', 'cancelled', 'skipped')),
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
    progress TEXT,
    UNIQUE(run_id, task_index),
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
);

INSERT INTO tasks_new SELECT * FROM tasks;

DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE INDEX idx_tasks_run_id ON tasks(run_id);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_executor_type ON tasks(executor_type);
CREATE INDEX idx_tasks_run_status ON tasks(run_id, status);
CREATE INDEX idx_tasks_run_task_name ON tasks(run_id, task_name);

-- Add 'skipped' to workflow_task_states last_status CHECK constraint.

CREATE TABLE workflow_task_states_new (
    id BLOB PRIMARY KEY,
    workflow_id BLOB NOT NULL,
    task_name TEXT NOT NULL,
    last_status TEXT NOT NULL CHECK (last_status IN ('pending', 'dispatched', 'running', 'paused', 'success', 'failed', 'cancelled', 'skipped')),
    last_run_id BLOB NOT NULL,
    last_task_id BLOB NOT NULL,
    last_output TEXT,
    last_error TEXT,
    last_finished_at TEXT,
    last_success_at TEXT,
    last_failure_at TEXT,
    updated_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(workflow_id, task_name),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

INSERT INTO workflow_task_states_new SELECT * FROM workflow_task_states;

DROP TABLE workflow_task_states;
ALTER TABLE workflow_task_states_new RENAME TO workflow_task_states;

CREATE INDEX idx_workflow_task_states_workflow_id ON workflow_task_states(workflow_id);
