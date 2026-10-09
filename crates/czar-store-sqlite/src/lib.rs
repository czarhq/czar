mod core;
mod deferred_jobs;
mod runs;
mod tasks;
mod tasks_fanout;
mod workflows;

pub use core::SqliteDatabase;

#[macro_use]
mod delegation;

impl_database_delegates!(SqliteDatabase, SqliteDatabase::run_migrations);
