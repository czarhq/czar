ALTER TABLE runs DROP CONSTRAINT IF EXISTS runs_status_check;
ALTER TABLE runs
    ADD CONSTRAINT runs_status_check
    CHECK (status IN ('pending', 'running', 'paused', 'success', 'failed', 'cancelled'));

ALTER TABLE tasks DROP CONSTRAINT IF EXISTS tasks_status_check;
ALTER TABLE tasks
    ADD CONSTRAINT tasks_status_check
    CHECK (status IN ('pending', 'paused', 'dispatched', 'running', 'success', 'failed', 'cancelled'));
