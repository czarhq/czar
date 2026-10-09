use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use tracing::info;

use czar_core::database::Database;

#[derive(Args, Default)]
pub struct Init;

impl Init {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        info!("Running database migrations...");
        db.run_migrations().await?;
        println!("✓ Database initialized successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests;
