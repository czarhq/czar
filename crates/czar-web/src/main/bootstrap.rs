use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, bail};
use clap::Parser;
use czar_core::database::Database;

#[cfg(feature = "postgres")]
use czar_store::PostgresDatabase;
#[cfg(feature = "sqlite")]
use czar_store::SqliteDatabase;
#[cfg(feature = "d1")]
use czar_store::d1::{D1Config, D1Database};

#[cfg(feature = "postgres")]
pub(crate) const DEFAULT_POSTGRES_DATABASE_URL: &str =
    "postgres://postgres:postgres@localhost:5432/orchestrator";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DatabaseBackend {
    Postgres,
    Sqlite,
    D1,
}

impl DatabaseBackend {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Sqlite => "sqlite",
            Self::D1 => "d1",
        }
    }
}

impl std::fmt::Display for DatabaseBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DatabaseBackend {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "postgres" => Ok(Self::Postgres),
            "sqlite" => Ok(Self::Sqlite),
            "d1" => Ok(Self::D1),
            _ => Err(format!(
                "unsupported backend '{value}'. Expected one of: postgres, sqlite, d1"
            )),
        }
    }
}

#[cfg_attr(not(feature = "d1"), allow(dead_code))]
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct D1RuntimeConfig {
    pub(crate) account_id: String,
    pub(crate) database_id: String,
    pub(crate) api_token: String,
    pub(crate) base_url: Option<String>,
}

#[cfg_attr(not(feature = "sqlite"), allow(dead_code))]
pub(crate) fn default_sqlite_database_url() -> String {
    if let Ok(home) = std::env::var("HOME") {
        return format!("sqlite://{home}/.czar/czar.db?mode=rwc");
    }
    "sqlite://./.czar/czar.db?mode=rwc".to_string()
}

#[derive(Parser, Debug, Default)]
#[command(name = "czar-web", about = "Serve the Czar web UI and API")]
pub(crate) struct Args {
    /// Database backend to use: postgres, sqlite, or d1
    #[arg(long)]
    pub(crate) db_backend: Option<DatabaseBackend>,
    /// Database connection string for postgres/sqlite backends
    #[arg(long)]
    pub(crate) database_url: Option<String>,
    /// Address to bind (e.g., 127.0.0.1:8080)
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub(crate) addr: String,
    /// Directory to scan for workflow definitions when bootstrapping/reloading
    #[arg(long)]
    pub(crate) workflows_dir: Option<String>,
    /// Cloudflare account ID for the d1 backend
    #[arg(long)]
    pub(crate) d1_account_id: Option<String>,
    /// Cloudflare D1 database ID
    #[arg(long)]
    pub(crate) d1_database_id: Option<String>,
    /// Cloudflare API token for D1
    #[arg(long)]
    pub(crate) d1_api_token: Option<String>,
    /// Override base URL for the D1 API
    #[arg(long)]
    pub(crate) d1_base_url: Option<String>,
    #[command(flatten)]
    pub(crate) auth: czar_web::auth::AuthArgs,
}

pub(crate) fn compiled_backends() -> Vec<DatabaseBackend> {
    vec![
        #[cfg(feature = "postgres")]
        DatabaseBackend::Postgres,
        #[cfg(feature = "sqlite")]
        DatabaseBackend::Sqlite,
        #[cfg(feature = "d1")]
        DatabaseBackend::D1,
    ]
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn ensure_backend_compiled(backend: DatabaseBackend) -> anyhow::Result<DatabaseBackend> {
    if compiled_backends().contains(&backend) {
        return Ok(backend);
    }

    bail!(
        "{backend} backend requested, but this czar-web build was not compiled with {backend} support."
    )
}

pub(crate) fn resolve_backend(
    cli_backend: Option<DatabaseBackend>,
) -> anyhow::Result<DatabaseBackend> {
    if let Some(backend) = cli_backend {
        return ensure_backend_compiled(backend);
    }

    if let Some(raw) = env_var("CZAR_DB_BACKEND") {
        let backend = DatabaseBackend::from_str(&raw)
            .map_err(|err| anyhow::anyhow!("invalid CZAR_DB_BACKEND value: {err}"))?;
        return ensure_backend_compiled(backend);
    }

    let compiled = compiled_backends();
    match compiled.as_slice() {
        [] => {
            bail!("No database backend enabled. Build czar-web with at least one backend feature.")
        }
        [backend] => Ok(*backend),
        _ => {
            let names = compiled
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "Multiple backends are enabled in this czar-web build ({names}). Set CZAR_DB_BACKEND or pass --db-backend to choose one."
            )
        }
    }
}

#[cfg_attr(not(any(feature = "postgres", feature = "sqlite")), allow(dead_code))]
pub(crate) fn resolve_database_url(
    cli_database_url: Option<String>,
    backend: DatabaseBackend,
) -> anyhow::Result<String> {
    if let Some(database_url) = cli_database_url.or_else(|| env_var("DATABASE_URL")) {
        return Ok(database_url);
    }

    match backend {
        DatabaseBackend::Postgres => {
            #[cfg(feature = "postgres")]
            {
                Ok(DEFAULT_POSTGRES_DATABASE_URL.to_string())
            }

            #[cfg(not(feature = "postgres"))]
            {
                bail!(
                    "Postgres backend requested, but this czar-web build was compiled without the 'postgres' feature."
                )
            }
        }
        DatabaseBackend::Sqlite => {
            #[cfg(feature = "sqlite")]
            {
                Ok(default_sqlite_database_url())
            }

            #[cfg(not(feature = "sqlite"))]
            {
                bail!(
                    "SQLite backend requested, but this czar-web build was compiled without the 'sqlite' feature."
                )
            }
        }
        DatabaseBackend::D1 => bail!(
            "D1 does not use DATABASE_URL. Set CZAR_D1_ACCOUNT_ID, CZAR_D1_DATABASE_ID, and CZAR_D1_API_TOKEN instead."
        ),
    }
}

#[cfg_attr(not(feature = "d1"), allow(dead_code))]
pub(crate) fn resolve_d1_config(args: &Args) -> anyhow::Result<D1RuntimeConfig> {
    fn require(name: &str, cli_value: &Option<String>) -> anyhow::Result<String> {
        cli_value
            .clone()
            .or_else(|| env_var(name))
            .with_context(|| format!("Missing D1 configuration value: {name}"))
    }

    let base_url = args
        .d1_base_url
        .clone()
        .or_else(|| env_var("CZAR_D1_BASE_URL"));

    Ok(D1RuntimeConfig {
        account_id: require("CZAR_D1_ACCOUNT_ID", &args.d1_account_id)?,
        database_id: require("CZAR_D1_DATABASE_ID", &args.d1_database_id)?,
        api_token: require("CZAR_D1_API_TOKEN", &args.d1_api_token)?,
        base_url,
    })
}

pub(crate) fn parse_addr(addr: &str) -> SocketAddr {
    addr.parse().expect("invalid address")
}

pub(crate) async fn init_database(args: &Args) -> anyhow::Result<Arc<dyn Database>> {
    match resolve_backend(args.db_backend)? {
        DatabaseBackend::Postgres => init_postgres_database(args).await,
        DatabaseBackend::Sqlite => init_sqlite_database(args).await,
        DatabaseBackend::D1 => init_d1_database(args).await,
    }
}

#[cfg(feature = "postgres")]
async fn init_postgres_database(args: &Args) -> anyhow::Result<Arc<dyn Database>> {
    let database_url = resolve_database_url(args.database_url.clone(), DatabaseBackend::Postgres)?;
    let db = Arc::new(PostgresDatabase::new(&database_url).await?);
    db.run_migrations().await?;
    Ok(db)
}

#[cfg(not(feature = "postgres"))]
async fn init_postgres_database(_args: &Args) -> anyhow::Result<Arc<dyn Database>> {
    bail!("Postgres backend requested, but this czar-web build does not include postgres support.")
}

#[cfg(feature = "sqlite")]
async fn init_sqlite_database(args: &Args) -> anyhow::Result<Arc<dyn Database>> {
    let database_url = resolve_database_url(args.database_url.clone(), DatabaseBackend::Sqlite)?;
    let db = Arc::new(SqliteDatabase::new(&database_url).await?);
    db.run_migrations().await?;
    Ok(db)
}

#[cfg(not(feature = "sqlite"))]
async fn init_sqlite_database(_args: &Args) -> anyhow::Result<Arc<dyn Database>> {
    bail!("SQLite backend requested, but this czar-web build does not include sqlite support.")
}

#[cfg(feature = "d1")]
async fn init_d1_database(args: &Args) -> anyhow::Result<Arc<dyn Database>> {
    let config = resolve_d1_config(args)?;
    let db = Arc::new(
        D1Database::new(&D1Config {
            account_id: config.account_id,
            database_id: config.database_id,
            api_token: config.api_token,
            base_url: config.base_url,
        })
        .await?,
    );
    db.run_migrations().await?;
    Ok(db)
}

#[cfg(not(feature = "d1"))]
async fn init_d1_database(_args: &Args) -> anyhow::Result<Arc<dyn Database>> {
    bail!("D1 backend requested, but this czar-web build does not include d1 support yet.")
}
