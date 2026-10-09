-- Add DAG workflow support (workflow YAML + task dependencies)

ALTER TABLE public.workflows
    ADD COLUMN workflow_yaml text,
    ADD COLUMN workflow_root text;

ALTER TABLE public.workflows
    DROP CONSTRAINT workflows_executor_type_check,
    ADD CONSTRAINT workflows_executor_type_check CHECK ((executor_type = ANY (ARRAY['cloudrun'::text, 'process'::text, 'python'::text])));

ALTER TABLE public.tasks
    ADD COLUMN task_name text,
    ADD COLUMN depends_on text[] DEFAULT '{}'::text[] NOT NULL;

UPDATE public.tasks
SET task_name = 'task_' || task_index
WHERE task_name IS NULL;

ALTER TABLE public.tasks
    ALTER COLUMN task_name SET NOT NULL;

CREATE INDEX idx_tasks_run_task_name ON public.tasks USING btree (run_id, task_name);
