-- Create workflow_snapshots table
CREATE TABLE workflow_snapshots (
    id BLOB PRIMARY KEY,
    workflow_id BLOB NOT NULL,
    content_hash TEXT NOT NULL,
    tasks_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

CREATE INDEX idx_workflow_snapshots_workflow_id ON workflow_snapshots(workflow_id);
CREATE UNIQUE INDEX idx_workflow_snapshots_content_hash ON workflow_snapshots(workflow_id, content_hash);

-- Add current_snapshot_id to workflows table
ALTER TABLE workflows ADD COLUMN current_snapshot_id BLOB;

-- Add snapshot_id to runs table
ALTER TABLE runs ADD COLUMN snapshot_id BLOB;
