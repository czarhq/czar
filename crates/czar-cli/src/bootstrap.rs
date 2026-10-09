use anyhow::bail;
use clap::Args;

#[cfg(feature = "d1")]
use czar_store::d1::{D1Config, D1Database};

pub(crate) use czar_store::bootstrap::{
    DatabaseBackend, load_project_env_from_current_dir, project_contract_validation_enabled_from,
    project_contract_validation_enabled_from_current_dir, project_notifications_from,
    resolve_database_url,
};

#[cfg_attr(not(feature = "d1"), allow(dead_code))]
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct D1RuntimeConfig {
    pub(crate) account_id: String,
    pub(crate) database_id: String,
    pub(crate) api_token: String,
    pub(crate) base_url: Option<String>,
}

#[derive(Args, Debug, Clone, Default)]
pub(crate) struct DatabaseArgs {
    /// Database backend to use: postgres, sqlite, or d1
    #[arg(long)]
    pub(crate) db_backend: Option<DatabaseBackend>,
    /// Database connection string for postgres/sqlite backends
    #[arg(long)]
    pub(crate) database_url: Option<String>,
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

fn ensure_backend_compiled(backend: DatabaseBackend) -> anyhow::Result<DatabaseBackend> {
    if compiled_backends().contains(&backend) {
        return Ok(backend);
    }

    bail!(
        "{backend} backend requested, but this czar-cli build was not compiled with {backend} support."
    )
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

/// The backend: `--db-backend`, else `CZAR_DB_BACKEND`, else the only one
/// compiled in. With several compiled in (the default build has SQLite and
/// Postgres), the database URL's scheme decides, and SQLite is the default.
pub(crate) fn resolve_backend(
    cli_backend: Option<DatabaseBackend>,
    database_url: Option<&str>,
) -> anyhow::Result<DatabaseBackend> {
    if let Some(backend) = cli_backend {
        return ensure_backend_compiled(backend);
    }

    if let Some(raw) = env_var("CZAR_DB_BACKEND") {
        let backend = raw
            .parse::<DatabaseBackend>()
            .map_err(|err| anyhow::anyhow!("invalid CZAR_DB_BACKEND value: {err}"))?;
        return ensure_backend_compiled(backend);
    }

    let compiled = compiled_backends();
    match compiled.as_slice() {
        [] => {
            bail!("No database backend enabled. Build czar-cli with at least one backend feature.")
        }
        [backend] => Ok(*backend),
        _ => {
            let url = database_url
                .map(str::to_string)
                .or_else(|| env_var("DATABASE_URL"));
            let by_url = match url.as_deref() {
                Some(u) if u.starts_with("postgres://") || u.starts_with("postgresql://") => {
                    Some(DatabaseBackend::Postgres)
                }
                Some(u) if u.starts_with("sqlite:") => Some(DatabaseBackend::Sqlite),
                _ => None,
            };
            if let Some(backend) = by_url.filter(|b| compiled.contains(b)) {
                return Ok(backend);
            }
            if url.is_none() && compiled.contains(&DatabaseBackend::Sqlite) {
                return Ok(DatabaseBackend::Sqlite);
            }
            let names = compiled
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "Multiple backends are enabled in this czar-cli build ({names}). Set CZAR_DB_BACKEND or pass --db-backend to choose one."
            )
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) async fn init_postgres_database(
    args: &DatabaseArgs,
) -> anyhow::Result<std::sync::Arc<czar_store::PostgresDatabase>> {
    let database_url = resolve_database_url(args.database_url.clone(), DatabaseBackend::Postgres)?;
    let pool_size: u32 = std::env::var("DB_POOL_SIZE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10);
    czar_store::bootstrap::init_postgres_database(&database_url, pool_size).await
}

#[cfg(feature = "sqlite")]
pub(crate) async fn init_sqlite_database(
    args: &DatabaseArgs,
) -> anyhow::Result<std::sync::Arc<czar_store::SqliteDatabase>> {
    let database_url = resolve_database_url(args.database_url.clone(), DatabaseBackend::Sqlite)?;
    czar_store::bootstrap::init_sqlite_database(&database_url).await
}

#[cfg_attr(not(feature = "d1"), allow(dead_code))]
pub(crate) fn resolve_d1_config(args: &DatabaseArgs) -> anyhow::Result<D1RuntimeConfig> {
    fn require(name: &str, cli_value: &Option<String>) -> anyhow::Result<String> {
        cli_value
            .clone()
            .or_else(|| env_var(name))
            .ok_or_else(|| anyhow::anyhow!("Missing D1 configuration value: {name}"))
    }

    Ok(D1RuntimeConfig {
        account_id: require("CZAR_D1_ACCOUNT_ID", &args.d1_account_id)?,
        database_id: require("CZAR_D1_DATABASE_ID", &args.d1_database_id)?,
        api_token: require("CZAR_D1_API_TOKEN", &args.d1_api_token)?,
        base_url: args
            .d1_base_url
            .clone()
            .or_else(|| env_var("CZAR_D1_BASE_URL")),
    })
}

#[cfg(feature = "d1")]
pub(crate) async fn init_d1_database(
    args: &DatabaseArgs,
) -> anyhow::Result<std::sync::Arc<D1Database>> {
    let config = resolve_d1_config(args)?;
    Ok(std::sync::Arc::new(
        D1Database::new(&D1Config {
            account_id: config.account_id,
            database_id: config.database_id,
            api_token: config.api_token,
            base_url: config.base_url,
        })
        .await?,
    ))
}
