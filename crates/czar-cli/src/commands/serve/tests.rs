use super::*;
use czar_store::SqliteDatabase;
use tokio::time::{Duration, sleep};
use uuid::Uuid;

#[tokio::test]
async fn test_serve_command_errors_on_missing_config_file() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    let result = Serve {
        config: Some("/tmp/does-not-exist-config.yaml".to_string()),
        ..serve_with_addr(free_local_addr())
    }
    .execute(db)
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_serve_command_errors_on_invalid_yaml_config() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    let path = std::env::temp_dir().join(format!("czar-serve-invalid-{}.yaml", Uuid::new_v4()));
    std::fs::write(&path, "this: [is: not-valid-yaml").expect("write invalid yaml");

    let result = Serve {
        config: Some(path.to_string_lossy().to_string()),
        ..serve_with_addr(free_local_addr())
    }
    .execute(db)
    .await;

    let _ = std::fs::remove_file(path);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_serve_command_uses_default_config() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    let handle = tokio::spawn(async move { serve_with_addr(free_local_addr()).execute(db).await });
    sleep(Duration::from_millis(100)).await;
    handle.abort();
    let join = handle.await;
    assert!(join.is_err(), "aborted serve should cancel task");
}

#[tokio::test]
async fn test_serve_command_uses_config_file() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");
    let path = std::env::temp_dir().join(format!("czar-serve-valid-{}.yaml", Uuid::new_v4()));
    std::fs::write(
        &path,
        r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 10
max_concurrent_dispatches: 2
max_concurrent_status_checks: 2
db_pool_size: 2
enable_triggerer: false
"#,
    )
    .expect("write valid yaml");

    let cfg_path = path.to_string_lossy().to_string();
    let handle = tokio::spawn(async move {
        Serve {
            config: Some(cfg_path),
            ..serve_with_addr(free_local_addr())
        }
        .execute(db)
        .await
    });
    sleep(Duration::from_millis(100)).await;
    handle.abort();
    let join = handle.await;
    let _ = std::fs::remove_file(path);
    assert!(join.is_err(), "aborted serve should cancel task");
}

#[tokio::test]
async fn test_serve_command_starts_web_server() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let addr = free_local_addr();
    let test_addr = addr.clone();

    let handle = tokio::spawn(async move { serve_with_addr(addr).execute(db).await });

    sleep(Duration::from_millis(200)).await;

    let response = reqwest::get(format!("http://{}/api/runs", test_addr))
        .await
        .expect("web request");
    assert!(response.status().is_success() || response.status().is_redirection());

    handle.abort();
}

#[tokio::test]
async fn test_serve_refuses_public_addr_without_auth_mode() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    let err = serve_with_addr("0.0.0.0:0".to_string())
        .execute(db)
        .await
        .expect_err("public addr without an auth mode");
    assert!(err.to_string().contains("--auth"), "{err}");
}

fn free_local_addr() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
    listener.local_addr().expect("local addr").to_string()
}

fn serve_with_addr(addr: String) -> Serve {
    Serve {
        addr,
        ..Serve::default()
    }
}

#[tokio::test]
async fn test_serve_command_returns_clear_error_when_addr_is_in_use() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
    db.run_migrations().await.expect("migrate");

    let addr = free_local_addr();
    let _occupied = std::net::TcpListener::bind(&addr).expect("occupy addr");
    let result = serve_with_addr(addr)
        .execute(db)
        .await
        .expect_err("occupied addr should fail");

    assert!(result.to_string().contains("address already in use"));
}

#[test]
fn worker_tokens_come_from_their_token_env() {
    use czar_core::worker_registry::WorkerConfig;

    let worker = |url: &str, token: Option<&str>, token_env: Option<&str>| WorkerConfig {
        url: url.to_string(),
        name: None,
        poll_interval_secs: None,
        token: token.map(str::to_string),
        token_env: token_env.map(str::to_string),
    };
    let mut workers = vec![
        worker("http://a:8081", None, Some("SALES_TOKEN")),
        worker("http://b:8081", Some("inline"), None),
        worker("http://c:8081", None, Some("MISSING_TOKEN")),
    ];
    let errors = super::workers::resolve_worker_tokens(&mut workers, |name| {
        (name == "SALES_TOKEN").then(|| "s3cret".to_string())
    });
    assert_eq!(workers[0].token.as_deref(), Some("s3cret"));
    assert_eq!(workers[1].token.as_deref(), Some("inline"));
    assert_eq!(workers[2].token, None);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("MISSING_TOKEN"), "{}", errors[0]);
}
