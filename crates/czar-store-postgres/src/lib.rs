mod core;
mod deferred_jobs;
mod runs;
mod tasks;
mod tasks_fanout;
mod workflows;

pub use core::PostgresDatabase;

#[macro_use]
mod delegation;

impl_database_delegates!(PostgresDatabase, PostgresDatabase::run_migrations);
