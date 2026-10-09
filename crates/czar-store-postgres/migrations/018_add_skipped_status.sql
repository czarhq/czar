-- Add 'skipped' to tasks and workflow_task_states status CHECK constraints.

ALTER TABLE tasks DROP CONSTRAINT IF EXISTS tasks_status_check;
ALTER TABLE tasks
    ADD CONSTRAINT tasks_status_check
    CHECK (status IN ('pending', 'paused', 'dispatched', 'running', 'success', 'failed', 'cancelled', 'skipped'));

ALTER TABLE workflow_task_states DROP CONSTRAINT IF EXISTS workflow_task_states_last_status_check;
ALTER TABLE workflow_task_states
    ADD CONSTRAINT workflow_task_states_last_status_check
    CHECK (last_status IN ('pending', 'dispatched', 'running', 'paused', 'success', 'failed', 'cancelled', 'skipped'));
