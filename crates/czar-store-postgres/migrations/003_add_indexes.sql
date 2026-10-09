-- Performance indexes for common query patterns

-- Index for finding workflows by executor type (for potential future filtering)
CREATE INDEX idx_workflows_executor_type ON workflows(executor_type);

-- Index for run status lookups (already exists: idx_runs_status)
-- Index for finding runs by workflow (already exists: idx_runs_workflow_id)

-- Composite index for finding pending tasks with run info efficiently
CREATE INDEX idx_tasks_status_run_id ON tasks(status, run_id) WHERE status IN ('pending', 'dispatched', 'running');

-- Index for checking task completion by run
CREATE INDEX idx_tasks_run_status ON tasks(run_id, status);

-- Index for finding tasks by execution name (for status polling)
CREATE INDEX idx_tasks_execution_name ON tasks(cloud_run_execution_name) WHERE cloud_run_execution_name IS NOT NULL;
