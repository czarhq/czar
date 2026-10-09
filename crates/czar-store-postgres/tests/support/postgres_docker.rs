#![allow(dead_code)]

//! Shared Postgres test harness for integration tests that need a real database.
//!
//! Behavior:
//! - If `CZAR_POSTGRES_TEST_URL` or `DATABASE_URL` is set, tests reuse that
//!   server.
//! - Otherwise the harness starts a disposable `postgres:16-alpine` container
//!   and waits for it to become ready.
//!
//! This harness intentionally does not inject faults by itself. Fault-injection
//! tests, such as the scheduler/Toxiproxy coverage in `czar-core`, layer a proxy
//! on top of this harness so ordinary Postgres integration tests stay simple.

use anyhow::{Context, Result, bail};
use sqlx::postgres::PgPoolOptions;
use std::net::{TcpListener, UdpSocket};
use std::process::Command;
use tokio::time::{Duration, sleep, timeout};
use url::Url;
use uuid::Uuid;

const POSTGRES_IMAGE: &str = "postgres:16-alpine";
const POSTGRES_USER: &str = "postgres";
const POSTGRES_PASSWORD: &str = "postgres";
const POSTGRES_DB: &str = "orchestrator";

pub struct PostgresServerHarness {
    base_url: String,
    managed_container_name: Option<String>,
}

impl PostgresServerHarness {
    pub async fn setup() -> Result<Option<Self>> {
        if let Some(url) = explicit_postgres_test_url() {
            wait_for_postgres(&with_database(&url, "postgres")?, 10).await?;
            return Ok(Some(Self {
                base_url: url,
                managed_container_name: None,
            }));
        }

        if let Err(e) = ensure_docker_available() {
            eprintln!("skipping Postgres integration test: {e}");
            return Ok(None);
        }

        let port = reserve_local_port()?;
        let container_name = format!("czar-test-postgres-{}", Uuid::new_v4().simple());
        let output = Command::new("docker")
            .args([
                "run",
                "-d",
                "--rm",
                "--name",
                &container_name,
                "-e",
                &format!("POSTGRES_USER={POSTGRES_USER}"),
                "-e",
                &format!("POSTGRES_PASSWORD={POSTGRES_PASSWORD}"),
                "-e",
                &format!("POSTGRES_DB={POSTGRES_DB}"),
                "-p",
                &format!("{port}:5432"),
                POSTGRES_IMAGE,
            ])
            .output()
            .context("failed to start postgres test container")?;
        if !output.status.success() {
            bail!(
                "failed to start postgres test container: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }

        let base_url = format!(
            "postgres://{POSTGRES_USER}:{POSTGRES_PASSWORD}@127.0.0.1:{port}/{POSTGRES_DB}"
        );
        if let Err(err) = wait_for_postgres(&with_database(&base_url, "postgres")?, 30).await {
            let _ = stop_container(&container_name);
            return Err(err.context("postgres test container did not become ready"));
        }

        Ok(Some(Self {
            base_url,
            managed_container_name: Some(container_name),
        }))
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn admin_url(&self) -> Result<String> {
        with_database(&self.base_url, "postgres")
    }

    pub fn upstream_for_containers(&self, database_url: &str) -> Result<String> {
        let url = Url::parse(database_url)?;
        let host = match url.host_str() {
            Some("localhost") | Some("127.0.0.1") => host_for_containers()?,
            Some(value) => value.to_string(),
            None => bail!("database URL is missing a host: {database_url}"),
        };
        Ok(format!("{}:{}", host, url.port().unwrap_or(5432)))
    }

    pub async fn cleanup(self) -> Result<()> {
        if let Some(container_name) = self.managed_container_name {
            stop_container(&container_name)?;
        }
        Ok(())
    }
}

pub fn with_database(base_url: &str, database: &str) -> Result<String> {
    let mut url = Url::parse(base_url)?;
    url.set_path(&format!("/{database}"));
    Ok(url.to_string())
}

fn explicit_postgres_test_url() -> Option<String> {
    std::env::var("CZAR_POSTGRES_TEST_URL")
        .ok()
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn reserve_local_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).context("failed to reserve local port")?;
    Ok(listener.local_addr()?.port())
}

fn ensure_docker_available() -> Result<()> {
    let output = Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .output()
        .context("failed to run docker version")?;
    if output.status.success() {
        return Ok(());
    }
    bail!(
        "docker is required for automated Postgres integration tests: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

fn stop_container(container_name: &str) -> Result<()> {
    let output = Command::new("docker")
        .args(["stop", container_name])
        .output()
        .with_context(|| format!("failed to stop docker container {container_name}"))?;
    if output.status.success() {
        return Ok(());
    }
    bail!(
        "failed to stop docker container {container_name}: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

fn host_for_containers() -> Result<String> {
    if let Ok(value) = std::env::var("CZAR_TEST_HOST_IP")
        && !value.is_empty()
    {
        return Ok(value);
    }

    if cfg!(target_os = "linux")
        && let Ok(output) = Command::new("sh")
            .arg("-c")
            .arg("ip -4 addr show docker0 2>/dev/null | awk '/inet / {print $2}' | cut -d/ -f1")
            .output()
        && output.status.success()
    {
        let docker_ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !docker_ip.is_empty() {
            return Ok(docker_ip);
        }
    }

    if cfg!(target_os = "linux") {
        let socket = UdpSocket::bind(("0.0.0.0", 0))?;
        socket.connect(("192.0.2.1", 80))?;
        return Ok(socket.local_addr()?.ip().to_string());
    }

    Ok("host.docker.internal".to_string())
}

async fn wait_for_postgres(admin_url: &str, timeout_secs: u64) -> Result<()> {
    timeout(Duration::from_secs(timeout_secs), async {
        loop {
            let connect = PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(1))
                .connect(admin_url)
                .await;
            match connect {
                Ok(pool) => {
                    pool.close().await;
                    break Ok::<(), anyhow::Error>(());
                }
                Err(_) => sleep(Duration::from_millis(250)).await,
            }
        }
    })
    .await
    .with_context(|| format!("timed out waiting for postgres at {admin_url}"))??;
    Ok(())
}
