-- Create workflow_snapshots table
CREATE TABLE workflow_snapshots (
    id UUID PRIMARY KEY,
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    tasks_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_workflow_snapshots_workflow_id ON workflow_snapshots(workflow_id);
CREATE UNIQUE INDEX idx_workflow_snapshots_content_hash ON workflow_snapshots(workflow_id, content_hash);

-- Add current_snapshot_id to workflows table
ALTER TABLE workflows ADD COLUMN current_snapshot_id UUID REFERENCES workflow_snapshots(id);

-- Add snapshot_id to runs table
ALTER TABLE runs ADD COLUMN snapshot_id UUID REFERENCES workflow_snapshots(id);
