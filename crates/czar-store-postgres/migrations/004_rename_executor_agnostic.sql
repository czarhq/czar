-- Rename cloud_run-specific columns to be executor-agnostic
ALTER TABLE workflows 
    RENAME COLUMN cloud_run_job_name TO job_name;

ALTER TABLE workflows 
    RENAME COLUMN cloud_run_project TO project;

ALTER TABLE workflows 
    RENAME COLUMN cloud_run_region TO region;

ALTER TABLE tasks 
    RENAME COLUMN cloud_run_execution_name TO execution_name;
