//! Scheduler integration tests that exercise database fault handling with a real
//! Postgres backend routed through Toxiproxy.
//!
//! Scope:
//! - These are integration tests for the scheduler's Postgres interaction path.
//! - They are not end-to-end UI tests and they do not verify user-facing UX.
//! - The scheduler under test talks to Postgres through Toxiproxy, while the
//!   assertions and setup code keep a direct Postgres connection so they can
//!   observe state even when the proxy is degraded or disabled.
//!
//! Faults currently covered:
//! - Full proxy disconnect via `enabled = false`, which simulates the scheduler
//!   losing database connectivity entirely for a short period.
//! - A downstream latency toxic of 1500 ms with zero jitter, which simulates a
//!   severe but deterministic database response slowdown.
//!
//! What these tests prove today:
//! - The scheduler task keeps running instead of crashing when the proxied
//!   database connection becomes unavailable or very slow.
//! - Once the injected fault is removed, the scheduler can resume normal
//!   processing and drive a pending DAG run to `success`.
//!
//! What these tests do not prove:
//! - User-facing behavior in the full stack during a fault.
//! - Behavior under packet loss, bandwidth throttling, connection resets,
//!   half-open sockets, or long-running repeated fault patterns.

#[path = "../../../../czar-store-postgres/tests/support/postgres_docker.rs"]
mod postgres_docker;

use super::support::{ExecMode, MockExecutor, MockExecutorManager, init_tracing, test_config};
use anyhow::{Context, Result, bail};
use czar_core::database::{NewWorkflowTask, RunRepository, WorkflowRepository};
use czar_core::scheduler::Scheduler;
use czar_store::PostgresDatabase;
use reqwest::Client;
use serde_json::json;
use std::net::SocketAddr;
use std::process::Command;
use std::sync::Arc;
use tokio::time::{Duration, sleep, timeout};
use url::Url;
use uuid::Uuid;

const TOXIPROXY_IMAGE: &str = "ghcr.io/shopify/toxiproxy:2.12.0";
const TOXIPROXY_CONTAINER_PORT: u16 = 15432;

struct PostgresToxiproxyContext {
    server: postgres_docker::PostgresServerHarness,
    admin: PostgresDatabase,
    db_name: String,
    direct_db: Arc<PostgresDatabase>,
    proxied_db: Arc<PostgresDatabase>,
    toxiproxy: ToxiproxyHarness,
}

impl PostgresToxiproxyContext {
    async fn setup() -> Result<Option<Self>> {
        let Some(server) = postgres_docker::PostgresServerHarness::setup().await? else {
            return Ok(None);
        };
        let admin = PostgresDatabase::new(&server.admin_url()?).await?;

        let db_name = format!("tox_{}", Uuid::new_v4().simple());
        let create_database_sql = format!(r#"CREATE DATABASE "{}""#, db_name);
        sqlx::query(sqlx::AssertSqlSafe(create_database_sql.as_str()))
            .execute(admin.pool())
            .await?;

        let db_url = postgres_docker::with_database(server.base_url(), &db_name)?;
        let direct_db = Arc::new(PostgresDatabase::new(&db_url).await?);
        direct_db.run_migrations().await?;

        let toxiproxy = match ToxiproxyHarness::start(&server, &db_url).await {
            Ok(harness) => harness,
            Err(err) => {
                eprintln!("Skipping toxiproxy scheduler test: {err}");
                direct_db.pool().close().await;
                let drop_database_sql = format!(r#"DROP DATABASE IF EXISTS "{}""#, db_name);
                let _ = sqlx::query(sqlx::AssertSqlSafe(drop_database_sql.as_str()))
                    .execute(admin.pool())
                    .await;
                let _ = server.cleanup().await;
                return Ok(None);
            }
        };

        let proxied_db = Arc::new(PostgresDatabase::new(&toxiproxy.database_url(&db_url)?).await?);

        Ok(Some(Self {
            server,
            admin,
            db_name,
            direct_db,
            proxied_db,
            toxiproxy,
        }))
    }

    async fn cleanup(self) -> Result<()> {
        self.proxied_db.pool().close().await;
        self.direct_db.pool().close().await;
        self.toxiproxy.stop().await;
        sqlx::query(
            "SELECT pg_terminate_backend(pid)
             FROM pg_stat_activity
             WHERE datname = $1 AND pid <> pg_backend_pid()",
        )
        .bind(&self.db_name)
        .execute(self.admin.pool())
        .await?;
        let drop_database_sql = format!(r#"DROP DATABASE IF EXISTS "{}""#, self.db_name);
        sqlx::query(sqlx::AssertSqlSafe(drop_database_sql.as_str()))
            .execute(self.admin.pool())
            .await?;
        self.server.cleanup().await?;
        Ok(())
    }
}

/// Manages a disposable Toxiproxy server plus a single Postgres proxy exposed on
/// localhost for the scheduler-under-test.
///
/// We control Toxiproxy over its HTTP API rather than shelling out to a client
/// binary so the test can create, mutate, and remove faults directly from Rust.
struct ToxiproxyHarness {
    container_name: String,
    proxy_name: String,
    api_url: String,
    proxy_addr: SocketAddr,
    client: Client,
}

impl ToxiproxyHarness {
    async fn start(
        server: &postgres_docker::PostgresServerHarness,
        database_url: &str,
    ) -> Result<Self> {
        let api_port = reserve_local_port()?;
        let proxy_port = reserve_local_port()?;
        let container_name = format!("czar-toxiproxy-{}", Uuid::new_v4().simple());
        let mut args = vec![
            "run".to_string(),
            "-d".to_string(),
            "--rm".to_string(),
            "--name".to_string(),
            container_name.clone(),
        ];
        if cfg!(target_os = "linux") {
            args.push("--add-host".to_string());
            args.push("host.docker.internal:host-gateway".to_string());
        }
        args.extend([
            "-p".to_string(),
            format!("{api_port}:8474"),
            "-p".to_string(),
            format!("{proxy_port}:{TOXIPROXY_CONTAINER_PORT}"),
            TOXIPROXY_IMAGE.to_string(),
        ]);

        let output = Command::new("docker")
            .args(&args)
            .output()
            .context("failed to start toxiproxy container")?;
        if !output.status.success() {
            bail!(
                "failed to start toxiproxy container: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }

        let client = Client::new();
        let api_url = format!("http://127.0.0.1:{api_port}");
        wait_for_toxiproxy_ready(&client, &api_url).await?;

        let proxy_name = format!("pg-{}", Uuid::new_v4().simple());
        let harness = Self {
            container_name,
            proxy_name: proxy_name.clone(),
            api_url,
            proxy_addr: SocketAddr::from(([127, 0, 0, 1], proxy_port)),
            client,
        };
        harness
            .create_proxy(server.upstream_for_containers(database_url)?)
            .await?;
        Ok(harness)
    }

    fn database_url(&self, database_url: &str) -> Result<String> {
        rewrite_database_url(database_url, self.proxy_addr)
    }

    async fn set_enabled(&self, enabled: bool) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/proxies/{}", self.api_url, self.proxy_name))
            .json(&json!({ "enabled": enabled }))
            .send()
            .await
            .context("failed to update toxiproxy enabled flag")?;
        ensure_success(response).await
    }

    async fn add_latency_toxic(&self, name: &str, latency_ms: u64) -> Result<()> {
        let response = self
            .client
            .post(format!(
                "{}/proxies/{}/toxics",
                self.api_url, self.proxy_name
            ))
            .json(&json!({
                "name": name,
                "type": "latency",
                "stream": "downstream",
                "toxicity": 1.0,
                "attributes": {
                    "latency": latency_ms,
                    "jitter": 0
                }
            }))
            .send()
            .await
            .context("failed to create toxiproxy latency toxic")?;
        ensure_success(response).await
    }

    async fn remove_toxic(&self, name: &str) -> Result<()> {
        let response = self
            .client
            .delete(format!(
                "{}/proxies/{}/toxics/{}",
                self.api_url, self.proxy_name, name
            ))
            .send()
            .await
            .context("failed to delete toxiproxy toxic")?;
        ensure_success(response).await
    }

    async fn stop(self) {
        let _ = Command::new("docker")
            .args(["stop", &self.container_name])
            .output();
    }

    async fn create_proxy(&self, upstream: String) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/proxies", self.api_url))
            .json(&json!({
                "name": self.proxy_name,
                "listen": format!("0.0.0.0:{TOXIPROXY_CONTAINER_PORT}"),
                "upstream": upstream,
                "enabled": true
            }))
            .send()
            .await
            .context("failed to create toxiproxy proxy")?;
        ensure_success(response).await
    }
}

async fn wait_for_toxiproxy_ready(client: &Client, api_url: &str) -> Result<()> {
    timeout(Duration::from_secs(10), async {
        loop {
            match client.get(format!("{api_url}/proxies")).send().await {
                Ok(response) if response.status().is_success() => break Ok::<(), anyhow::Error>(()),
                _ => sleep(Duration::from_millis(100)).await,
            }
        }
    })
    .await
    .context("timed out waiting for toxiproxy API")??;
    Ok(())
}

async fn ensure_success(response: reqwest::Response) -> Result<()> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let body = response.text().await.unwrap_or_default();
    bail!("toxiproxy API request failed with {status}: {body}");
}

fn reserve_local_port() -> Result<u16> {
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).context("failed to reserve local port")?;
    Ok(listener.local_addr()?.port())
}

fn rewrite_database_url(database_url: &str, proxy_addr: SocketAddr) -> Result<String> {
    let mut url = Url::parse(database_url)?;
    url.set_host(Some("127.0.0.1"))?;
    url.set_port(Some(proxy_addr.port()))
        .map_err(|()| anyhow::anyhow!("failed to set proxy port on database URL"))?;
    Ok(url.to_string())
}

async fn create_pending_dag_run(db: &PostgresDatabase, name: &str) -> Result<Uuid> {
    let workflow = db
        .create_workflow(name, None, "dag", "local", "local", "dag", None, None)
        .await?;
    db.create_workflow_tasks(
        workflow.id,
        &[NewWorkflowTask {
            task_index: 0,
            task_name: "task-a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo hi"}),
            signature: None,
        }],
    )
    .await?;
    Ok(db.create_run(workflow.id, "test").await?.id)
}

async fn wait_for_run_status(
    db: &PostgresDatabase,
    run_id: Uuid,
    expected: &str,
    timeout_secs: u64,
) -> Result<()> {
    timeout(Duration::from_secs(timeout_secs), async {
        loop {
            let run = db.get_run(run_id).await?;
            if run.status_str() == expected {
                break Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    Ok(())
}

#[tokio::test]
#[ignore = "requires Docker for toxiproxy and a disposable Postgres container"]
async fn test_scheduler_recovers_after_toxiproxy_disconnect() -> Result<()> {
    let _trace = init_tracing();
    let Some(ctx) = PostgresToxiproxyContext::setup().await? else {
        return Ok(());
    };

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(ctx.proxied_db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let result = async {
        // Simulate a hard DB outage by disabling the proxy entirely, then
        // verify the scheduler task stays alive and recovers once connectivity
        // is restored.
        ctx.toxiproxy.set_enabled(false).await?;
        let run_id = create_pending_dag_run(ctx.direct_db.as_ref(), "toxiproxy-disconnect").await?;
        sleep(Duration::from_millis(250)).await;
        assert!(
            !handle.is_finished(),
            "scheduler should keep running while the database proxy is down"
        );

        ctx.toxiproxy.set_enabled(true).await?;
        wait_for_run_status(ctx.direct_db.as_ref(), run_id, "success", 10).await
    }
    .await;
    handle.abort();
    let _ = handle.await;
    let cleanup = ctx.cleanup().await;
    result?;
    cleanup?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires Docker for toxiproxy and a disposable Postgres container"]
async fn test_scheduler_recovers_after_toxiproxy_latency_spike() -> Result<()> {
    let _trace = init_tracing();
    let Some(ctx) = PostgresToxiproxyContext::setup().await? else {
        return Ok(());
    };

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(ctx.proxied_db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let result = async {
        // Simulate a severe but deterministic slowdown on DB responses, then
        // verify the scheduler keeps running and resumes normal processing once
        // the latency toxic is removed.
        ctx.toxiproxy.add_latency_toxic("db-latency", 1_500).await?;
        let run_id = create_pending_dag_run(ctx.direct_db.as_ref(), "toxiproxy-latency").await?;
        sleep(Duration::from_millis(200)).await;
        assert!(
            !handle.is_finished(),
            "scheduler should keep running during a database latency spike"
        );

        ctx.toxiproxy.remove_toxic("db-latency").await?;
        wait_for_run_status(ctx.direct_db.as_ref(), run_id, "success", 10).await
    }
    .await;
    handle.abort();
    let _ = handle.await;
    let cleanup = ctx.cleanup().await;
    result?;
    cleanup?;
    Ok(())
}
