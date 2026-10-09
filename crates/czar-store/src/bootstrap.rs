use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, bail};
use czar_core::notifications::NotificationsConfig;
use serde::Deserialize;

#[cfg(feature = "postgres")]
use crate::PostgresDatabase;
#[cfg(feature = "sqlite")]
use crate::SqliteDatabase;

#[cfg(feature = "postgres")]
pub const DEFAULT_POSTGRES_DATABASE_URL: &str =
    "postgres://postgres:postgres@localhost:5432/orchestrator";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseBackend {
    Postgres,
    Sqlite,
    D1,
}

impl DatabaseBackend {
    pub fn as_str(self) -> &'static str {
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

#[derive(Debug, Default, Deserialize)]
struct ProjectConfig {
    env_file: Option<PathBuf>,
    validation: Option<ProjectValidationConfig>,
    notifications: Option<NotificationsConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct ProjectValidationConfig {
    enabled: Option<bool>,
}

#[cfg_attr(not(feature = "sqlite"), allow(dead_code))]
pub fn default_sqlite_database_url() -> String {
    if let Ok(home) = std::env::var("HOME") {
        return format!("sqlite://{home}/.czar/czar.db?mode=rwc");
    }
    "sqlite://./.czar/czar.db?mode=rwc".to_string()
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn find_project_config(start_dir: &Path) -> Option<PathBuf> {
    start_dir
        .ancestors()
        .map(|dir| dir.join("czar.yaml"))
        .find(|path| path.is_file())
}

fn load_project_config(path: &Path) -> anyhow::Result<ProjectConfig> {
    let config_content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&config_content)
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn parse_env_assignment(line: &str) -> anyhow::Result<Option<(String, String)>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let without_export = trimmed
        .strip_prefix("export ")
        .map(str::trim_start)
        .unwrap_or(trimmed);
    let Some((raw_key, raw_value)) = without_export.split_once('=') else {
        bail!("invalid env assignment '{trimmed}': expected KEY=VALUE");
    };

    let key = raw_key.trim();
    if key.is_empty() {
        bail!("invalid env assignment '{trimmed}': missing key");
    }

    let value = raw_value.trim();
    let value = if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    };

    Ok(Some((key.to_string(), value)))
}

fn load_env_file(path: &Path) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    for (index, line) in content.lines().enumerate() {
        let Some((key, value)) = parse_env_assignment(line)
            .with_context(|| format!("invalid {}:{} in env file", path.display(), index + 1))?
        else {
            continue;
        };
        if std::env::var_os(&key).is_none() {
            unsafe {
                std::env::set_var(&key, value);
            }
        }
    }
    Ok(())
}

pub fn load_project_env_from(start_dir: &Path) -> anyhow::Result<Option<PathBuf>> {
    let Some(config_path) = find_project_config(start_dir) else {
        return Ok(None);
    };
    let config = load_project_config(&config_path)?;
    let Some(env_file) = config.env_file else {
        return Ok(None);
    };
    let env_path = config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(env_file);
    load_env_file(&env_path)?;
    Ok(Some(env_path))
}

pub fn load_project_env_from_current_dir() -> anyhow::Result<Option<PathBuf>> {
    let cwd =
        std::env::current_dir().with_context(|| "failed to determine current working directory")?;
    load_project_env_from(&cwd)
}

pub fn project_contract_validation_enabled_from(start_dir: &Path) -> anyhow::Result<bool> {
    let Some(config_path) = find_project_config(start_dir) else {
        return Ok(true);
    };
    let config = load_project_config(&config_path)?;
    Ok(config
        .validation
        .and_then(|validation| validation.enabled)
        .unwrap_or(true))
}

pub fn project_contract_validation_enabled_from_current_dir() -> anyhow::Result<bool> {
    let cwd =
        std::env::current_dir().with_context(|| "failed to determine current working directory")?;
    project_contract_validation_enabled_from(&cwd)
}

pub fn project_notifications_from(start_dir: &Path) -> anyhow::Result<NotificationsConfig> {
    let Some(config_path) = find_project_config(start_dir) else {
        return Ok(NotificationsConfig::default());
    };
    let config = load_project_config(&config_path)?;
    Ok(config.notifications.unwrap_or_default())
}

#[allow(dead_code)]
pub fn resolve_database_url(
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
                    "Postgres backend requested, but this build was not compiled with the 'postgres' feature."
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
                    "SQLite backend requested, but this build was not compiled with the 'sqlite' feature."
                )
            }
        }
        DatabaseBackend::D1 => bail!(
            "D1 does not use DATABASE_URL. Set CZAR_D1_ACCOUNT_ID, CZAR_D1_DATABASE_ID, and CZAR_D1_API_TOKEN instead."
        ),
    }
}

#[cfg(feature = "postgres")]
pub async fn init_postgres_database(
    database_url: &str,
    pool_size: u32,
) -> anyhow::Result<std::sync::Arc<PostgresDatabase>> {
    Ok(std::sync::Arc::new(
        PostgresDatabase::new_with_pool_size(database_url, pool_size).await?,
    ))
}

#[cfg(feature = "sqlite")]
pub async fn init_sqlite_database(
    database_url: &str,
) -> anyhow::Result<std::sync::Arc<SqliteDatabase>> {
    Ok(std::sync::Arc::new(
        SqliteDatabase::new(database_url).await?,
    ))
}
