ALTER TABLE runs
    ADD COLUMN task_selection TEXT;

CREATE TABLE workflow_task_states (
    id BLOB PRIMARY KEY,
    workflow_id BLOB NOT NULL,
    task_name TEXT NOT NULL,
    last_status TEXT NOT NULL CHECK (last_status IN ('pending', 'dispatched', 'running', 'paused', 'success', 'failed', 'cancelled')),
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

CREATE INDEX idx_workflow_task_states_workflow_id
    ON workflow_task_states(workflow_id);
