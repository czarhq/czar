-- Add executor type to workflows
ALTER TABLE workflows ADD COLUMN executor_type TEXT NOT NULL DEFAULT 'cloudrun' CHECK (executor_type IN ('cloudrun', 'process'));
