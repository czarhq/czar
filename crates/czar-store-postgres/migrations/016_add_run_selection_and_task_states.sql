ALTER TABLE runs
    ADD COLUMN task_selection JSONB;

CREATE TABLE workflow_task_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    task_name TEXT NOT NULL,
    last_status TEXT NOT NULL CHECK (last_status IN ('pending', 'dispatched', 'running', 'paused', 'success', 'failed', 'cancelled')),
    last_run_id UUID NOT NULL,
    last_task_id UUID NOT NULL,
    last_output JSONB,
    last_error TEXT,
    last_finished_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    last_failure_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT workflow_task_states_workflow_task_unique UNIQUE (workflow_id, task_name)
);

CREATE INDEX idx_workflow_task_states_workflow_id
    ON workflow_task_states (workflow_id);
