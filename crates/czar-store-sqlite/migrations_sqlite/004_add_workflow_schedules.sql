-- Add schedule support to workflows
ALTER TABLE workflows ADD COLUMN schedule TEXT;
ALTER TABLE workflows ADD COLUMN schedule_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE workflows ADD COLUMN last_scheduled_at TEXT;
ALTER TABLE workflows ADD COLUMN next_scheduled_at TEXT;

-- Index for finding workflows that need to be triggered
CREATE INDEX idx_workflows_next_scheduled ON workflows(next_scheduled_at) WHERE schedule_enabled = 1;
