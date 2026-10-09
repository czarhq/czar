use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use super::Serve;
use czar_core::database::Database;
use czar_web::auth::AuthArgs;

#[derive(Args, Default)]
pub struct Dev {
    /// Optional config file path (YAML)
    #[arg(short, long)]
    pub config: Option<String>,

    /// Address to bind the API server (e.g., 127.0.0.1:4000)
    #[arg(long, default_value = "127.0.0.1:4000")]
    pub addr: String,

    /// Directory to scan for workflow definitions when bootstrapping/reloading
    #[arg(long)]
    pub workflows_dir: Option<String>,

    #[command(flatten)]
    pub auth: AuthArgs,
}

impl Dev {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        Serve {
            config: self.config,
            addr: self.addr,
            workflows_dir: self.workflows_dir,
            workers_file: None,
            allowed_origins: Vec::new(),
            auth: self.auth,
        }
        .execute(db)
        .await
    }
}
