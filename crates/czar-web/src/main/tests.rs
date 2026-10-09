use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};

use super::bootstrap::{
    Args, D1RuntimeConfig, DEFAULT_POSTGRES_DATABASE_URL, DatabaseBackend, compiled_backends,
    parse_addr, resolve_backend, resolve_d1_config, resolve_database_url,
};
use super::{init_tracing, run_api_server, run_from_args};
use clap::Parser;
use czar_core::database::Database;
use czar_store::SqliteDatabase;
use czar_web::api::ApiServer;
use tokio::sync::oneshot;
use tokio::time::{Duration, sleep};

#[cfg(feature = "sqlite")]
use super::bootstrap::default_sqlite_database_url;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn free_local_addr() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
    listener.local_addr().expect("local addr")
}

#[test]
fn test_args_defaults() {
    let args = Args::parse_from(["czar-web"]);
    assert_eq!(args.db_backend, None);
    assert_eq!(args.database_url, None);
    assert_eq!(args.addr, "127.0.0.1:8080");
    assert_eq!(args.workflows_dir, None);
    assert_eq!(args.d1_account_id, None);
    assert_eq!(args.d1_database_id, None);
    assert_eq!(args.d1_api_token, None);
    assert_eq!(args.d1_base_url, None);
}

#[test]
fn test_database_backend_parser_accepts_supported_values() {
    assert_eq!(
        DatabaseBackend::from_str("postgres").expect("postgres backend"),
        DatabaseBackend::Postgres
    );
    assert_eq!(
        DatabaseBackend::from_str("SQLITE").expect("sqlite backend"),
        DatabaseBackend::Sqlite
    );
    assert_eq!(
        DatabaseBackend::from_str(" d1 ").expect("d1 backend"),
        DatabaseBackend::D1
    );
}

#[test]
fn test_database_backend_parser_rejects_invalid_value() {
    let err = DatabaseBackend::from_str("mysql").expect_err("invalid backend should fail");
    assert!(err.contains("unsupported backend"));
}

#[test]
fn test_resolve_database_url_env_override_and_fallback() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("DATABASE_URL").ok();
    unsafe {
        std::env::set_var("DATABASE_URL", "postgres://env-db");
    }
    assert_eq!(
        resolve_database_url(None, DatabaseBackend::Postgres).expect("database url"),
        "postgres://env-db"
    );

    unsafe {
        std::env::remove_var("DATABASE_URL");
    }

    #[cfg(feature = "postgres")]
    assert_eq!(
        resolve_database_url(None, DatabaseBackend::Postgres).expect("postgres default url"),
        DEFAULT_POSTGRES_DATABASE_URL
    );

    #[cfg(feature = "sqlite")]
    assert_eq!(
        resolve_database_url(None, DatabaseBackend::Sqlite).expect("sqlite default url"),
        default_sqlite_database_url()
    );

    match prev {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
}

#[test]
fn test_resolve_database_url_prefers_cli_value() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("DATABASE_URL").ok();
    unsafe {
        std::env::set_var("DATABASE_URL", "postgres://env-db");
    }
    let Some(backend) = compiled_backends()
        .into_iter()
        .find(|backend| *backend != DatabaseBackend::D1)
    else {
        match prev {
            Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
            None => unsafe { std::env::remove_var("DATABASE_URL") },
        }
        return;
    };

    let resolved =
        resolve_database_url(Some("postgres://cli-db".to_string()), backend).expect("database url");
    assert_eq!(resolved, "postgres://cli-db");

    match prev {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
}

#[test]
fn test_resolve_database_url_for_d1_requires_d1_config_vars() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("DATABASE_URL").ok();
    unsafe {
        std::env::remove_var("DATABASE_URL");
    }

    let err = resolve_database_url(None, DatabaseBackend::D1)
        .expect_err("d1 should not use database url");
    assert!(err.to_string().contains("D1 does not use DATABASE_URL"));

    match prev {
        Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
        None => unsafe { std::env::remove_var("DATABASE_URL") },
    }
}

#[test]
fn test_resolve_backend_prefers_cli_over_env() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("CZAR_DB_BACKEND").ok();
    let explicit_backend = compiled_backends()
        .first()
        .copied()
        .expect("compiled backend");
    unsafe {
        std::env::set_var("CZAR_DB_BACKEND", "invalid-backend");
    }

    let backend = resolve_backend(Some(explicit_backend)).expect("backend");
    assert_eq!(backend, explicit_backend);

    match prev {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[test]
fn test_resolve_backend_reads_env() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("CZAR_DB_BACKEND").ok();
    let expected_backend = compiled_backends()
        .first()
        .copied()
        .expect("compiled backend");
    unsafe {
        std::env::set_var("CZAR_DB_BACKEND", expected_backend.as_str());
    }

    let backend = resolve_backend(None).expect("backend");
    assert_eq!(backend, expected_backend);

    match prev {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[test]
fn test_resolve_backend_rejects_invalid_env_value() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("CZAR_DB_BACKEND").ok();
    unsafe {
        std::env::set_var("CZAR_DB_BACKEND", "mysql");
    }

    let err = resolve_backend(None).expect_err("invalid env backend should fail");
    assert!(err.to_string().contains("invalid CZAR_DB_BACKEND value"));

    match prev {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[test]
fn test_resolve_backend_defaults_to_only_compiled_backend() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("CZAR_DB_BACKEND").ok();
    unsafe {
        std::env::remove_var("CZAR_DB_BACKEND");
    }

    let compiled = compiled_backends();
    if compiled.len() == 1 {
        let backend = resolve_backend(None).expect("backend");
        assert_eq!(backend, compiled[0]);
    }

    match prev {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[cfg(all(feature = "postgres", feature = "sqlite"))]
#[test]
fn test_resolve_backend_requires_explicit_selection_when_multiple_compiled() {
    let _guard = env_lock().lock().expect("env lock");
    let prev = std::env::var("CZAR_DB_BACKEND").ok();
    unsafe {
        std::env::remove_var("CZAR_DB_BACKEND");
    }

    let err =
        resolve_backend(None).expect_err("multiple compiled backends should require selection");
    assert!(
        err.to_string()
            .contains("Multiple backends are enabled in this czar-web build")
    );

    match prev {
        Some(v) => unsafe { std::env::set_var("CZAR_DB_BACKEND", v) },
        None => unsafe { std::env::remove_var("CZAR_DB_BACKEND") },
    }
}

#[test]
fn test_resolve_d1_config_reads_cli_and_env_values() {
    let _guard = env_lock().lock().expect("env lock");
    let prev_account = std::env::var("CZAR_D1_ACCOUNT_ID").ok();
    let prev_database = std::env::var("CZAR_D1_DATABASE_ID").ok();
    let prev_token = std::env::var("CZAR_D1_API_TOKEN").ok();
    let prev_base = std::env::var("CZAR_D1_BASE_URL").ok();
    unsafe {
        std::env::set_var("CZAR_D1_ACCOUNT_ID", "env-account");
        std::env::set_var("CZAR_D1_DATABASE_ID", "env-database");
        std::env::set_var("CZAR_D1_API_TOKEN", "env-token");
        std::env::set_var("CZAR_D1_BASE_URL", "https://env-base.invalid");
    }

    let args = Args {
        db_backend: Some(DatabaseBackend::D1),
        database_url: None,
        addr: "127.0.0.1:8080".to_string(),
        workflows_dir: None,
        d1_account_id: Some("cli-account".to_string()),
        d1_database_id: None,
        d1_api_token: None,
        d1_base_url: Some("https://cli-base.invalid".to_string()),
        ..Default::default()
    };
    let config = resolve_d1_config(&args).expect("d1 config");
    assert_eq!(
        config,
        D1RuntimeConfig {
            account_id: "cli-account".to_string(),
            database_id: "env-database".to_string(),
            api_token: "env-token".to_string(),
            base_url: Some("https://cli-base.invalid".to_string()),
        }
    );

    match prev_account {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_ACCOUNT_ID", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_ACCOUNT_ID") },
    }
    match prev_database {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_DATABASE_ID", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_DATABASE_ID") },
    }
    match prev_token {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_API_TOKEN", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_API_TOKEN") },
    }
    match prev_base {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_BASE_URL", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_BASE_URL") },
    }
}

#[test]
fn test_resolve_d1_config_requires_mandatory_fields() {
    let _guard = env_lock().lock().expect("env lock");
    let prev_account = std::env::var("CZAR_D1_ACCOUNT_ID").ok();
    let prev_database = std::env::var("CZAR_D1_DATABASE_ID").ok();
    let prev_token = std::env::var("CZAR_D1_API_TOKEN").ok();
    unsafe {
        std::env::remove_var("CZAR_D1_ACCOUNT_ID");
        std::env::remove_var("CZAR_D1_DATABASE_ID");
        std::env::remove_var("CZAR_D1_API_TOKEN");
    }

    let args = Args {
        db_backend: Some(DatabaseBackend::D1),
        database_url: None,
        addr: "127.0.0.1:8080".to_string(),
        workflows_dir: None,
        d1_account_id: None,
        d1_database_id: None,
        d1_api_token: None,
        d1_base_url: None,
        ..Default::default()
    };
    let err = resolve_d1_config(&args).expect_err("missing d1 config should fail");
    assert!(err.to_string().contains("CZAR_D1_ACCOUNT_ID"));

    match prev_account {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_ACCOUNT_ID", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_ACCOUNT_ID") },
    }
    match prev_database {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_DATABASE_ID", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_DATABASE_ID") },
    }
    match prev_token {
        Some(v) => unsafe { std::env::set_var("CZAR_D1_API_TOKEN", v) },
        None => unsafe { std::env::remove_var("CZAR_D1_API_TOKEN") },
    }
}

#[test]
fn test_parse_addr() {
    let addr = parse_addr("127.0.0.1:4000");
    assert_eq!(addr, "127.0.0.1:4000".parse::<SocketAddr>().expect("addr"));
}

#[test]
#[should_panic(expected = "invalid address")]
fn test_parse_addr_panics_on_invalid_input() {
    let _ = parse_addr("definitely-not-an-address");
}

#[tokio::test]
async fn test_run_api_server_returns_error_when_bind_fails() {
    let addr = free_local_addr();
    let _occupied = std::net::TcpListener::bind(addr).expect("occupy addr");
    let result = czar_web::api::bind_api_listener(addr).await;
    let err = result.expect_err("bind on occupied addr should fail");
    assert!(err.to_string().contains("address already in use"));
}

#[tokio::test]
async fn test_spa_routing_with_ui_bundle() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
    db.run_migrations().await.expect("migrations");
    let db: Arc<dyn Database> = db;

    let dist = std::env::temp_dir().join(format!("czar-web-spa-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::write(dist.join("index.html"), "<html></html>").unwrap();
    std::fs::write(dist.join("config.js"), "window.CZAR_CONFIG={}").unwrap();
    let bundle = czar_web::ui_bundle::UiBundle::new(dist.clone()).expect("bundle");

    let addr = free_local_addr();
    let listener = czar_web::api::bind_api_listener(addr)
        .await
        .expect("bind listener");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let api = czar_web::api::ApiServer::new_with_workflow_definitions_root(
        db,
        std::env::current_dir().expect("cwd"),
    );
    let app = czar_web::api::build_router_with_ui_bundle(api, Some(bundle));
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
    });

    let root_response = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match reqwest::get(format!("http://{addr}/")).await {
                Ok(resp) => break resp,
                Err(_) => sleep(Duration::from_millis(50)).await,
            }
        }
    })
    .await
    .expect("http timeout");

    assert_eq!(root_response.status().as_u16(), 200);

    let _ = shutdown_tx.send(());
    let _ = server.await;
    let _ = std::fs::remove_dir_all(&dist);
}

#[tokio::test]
async fn test_run_api_server_serves_api_and_spa() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
    db.run_migrations().await.expect("migrations");
    let db: Arc<dyn Database> = db;

    let addr = free_local_addr();
    let listener = czar_web::api::bind_api_listener(addr)
        .await
        .expect("bind listener");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        run_api_server(
            ApiServer::new_with_workflow_definitions_root(
                db,
                std::env::current_dir().expect("cwd"),
            ),
            listener,
            async move {
                let _ = shutdown_rx.await;
            },
        )
        .await
    });

    let response = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match reqwest::get(format!("http://{addr}/api/runs")).await {
                Ok(resp) => break resp,
                Err(_) => sleep(Duration::from_millis(50)).await,
            }
        }
    })
    .await
    .expect("http timeout");

    assert!(response.status().is_success());

    let _ = shutdown_tx.send(());
    let result = server.await.expect("join server");
    assert!(result.is_ok());
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn test_run_from_args_serves_api_and_spa() {
    init_tracing();
    let addr = free_local_addr();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let args = Args {
        db_backend: Some(DatabaseBackend::Sqlite),
        database_url: Some(":memory:".to_string()),
        addr: addr.to_string(),
        workflows_dir: None,
        d1_account_id: None,
        d1_database_id: None,
        d1_api_token: None,
        d1_base_url: None,
        ..Default::default()
    };

    let server = tokio::spawn(async move {
        run_from_args(args, async move {
            let _ = shutdown_rx.await;
        })
        .await
    });

    let response = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match reqwest::get(format!("http://{addr}/api/runs")).await {
                Ok(resp) => break resp,
                Err(_) => sleep(Duration::from_millis(50)).await,
            }
        }
    })
    .await
    .expect("http timeout");
    assert!(response.status().is_success());

    let _ = shutdown_tx.send(());
    let result = server.await.expect("join server");
    assert!(result.is_ok());
}

#[cfg(all(feature = "postgres", not(feature = "sqlite")))]
#[tokio::test]
async fn test_run_from_args_errors_when_database_cannot_connect() {
    init_tracing();
    let args = Args {
        db_backend: Some(DatabaseBackend::Postgres),
        database_url: Some("postgres://postgres:postgres@127.0.0.1:1/czar".to_string()),
        addr: free_local_addr().to_string(),
        workflows_dir: None,
        d1_account_id: None,
        d1_database_id: None,
        d1_api_token: None,
        d1_base_url: None,
        ..Default::default()
    };

    let result = run_from_args(args, async {}).await;
    assert!(result.is_err(), "expected DB connect failure");
}
