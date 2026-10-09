// FileDatabase - File-based implementation of the Database trait
//
// This stores workflows, runs, and tasks as JSON files in a directory structure:
// - workflows/{workflow_id}.json
// - runs/{run_id}.json
// - tasks/{task_id}.json

mod core;
mod deferred_jobs;
mod runs;
mod tasks;
mod tasks_fanout;
mod tasks_workflow_states;
mod workflows;

pub use core::FileDatabase;

#[macro_use]
mod delegation;

impl_database_delegates!(FileDatabase, FileDatabase::run_migrations);
