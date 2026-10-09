-- Add schedule support to workflows
ALTER TABLE workflows ADD COLUMN schedule TEXT;
ALTER TABLE workflows ADD COLUMN schedule_enabled BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE workflows ADD COLUMN last_scheduled_at TIMESTAMPTZ;
ALTER TABLE workflows ADD COLUMN next_scheduled_at TIMESTAMPTZ;

-- Index for finding workflows that need to be triggered
CREATE INDEX idx_workflows_next_scheduled ON workflows(next_scheduled_at) WHERE schedule_enabled = true;
