-- Store compiled workflow DAG and per-task executor selection

ALTER TABLE public.workflows
    DROP COLUMN workflow_yaml,
    DROP COLUMN workflow_root;

-- Allow any executor type at workflow level (used as default/label)
ALTER TABLE public.workflows
    DROP CONSTRAINT workflows_executor_type_check;

ALTER TABLE public.tasks
    ADD COLUMN executor_type text;

UPDATE public.tasks t
SET executor_type = w.executor_type
FROM public.runs r
JOIN public.workflows w ON r.workflow_id = w.id
WHERE t.run_id = r.id AND t.executor_type IS NULL;

UPDATE public.tasks
SET executor_type = 'process'
WHERE executor_type IS NULL;

ALTER TABLE public.tasks
    ALTER COLUMN executor_type SET NOT NULL;

CREATE INDEX idx_tasks_executor_type ON public.tasks USING btree (executor_type);

CREATE TABLE public.workflow_tasks (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    workflow_id uuid NOT NULL,
    task_index integer NOT NULL,
    task_name text NOT NULL,
    executor_type text NOT NULL,
    depends_on text[] DEFAULT '{}'::text[] NOT NULL,
    params jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT workflow_tasks_pkey PRIMARY KEY (id),
    CONSTRAINT workflow_tasks_workflow_id_task_name_key UNIQUE (workflow_id, task_name)
);

ALTER TABLE ONLY public.workflow_tasks
    ADD CONSTRAINT workflow_tasks_workflow_id_fkey FOREIGN KEY (workflow_id) REFERENCES public.workflows(id) ON DELETE CASCADE;

CREATE INDEX idx_workflow_tasks_workflow_id ON public.workflow_tasks USING btree (workflow_id);
