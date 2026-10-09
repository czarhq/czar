//! The request guard through the real router: `/api` and `/ws` requests from
//! another website, or with a rebound `Host`, are rejected before any handler.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

use czar_store::SqliteDatabase;
use czar_web::api::{ApiServer, build_router};

#[tokio::test]
async fn cross_site_post_is_rejected_before_the_handler() {
    let app = setup().await;
    let status = send(
        &app,
        Method::POST,
        "/api/workflows/reload",
        "127.0.0.1:4000",
        Some("https://evil.example"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cross_site_websocket_is_rejected() {
    let app = setup().await;
    let status = send(
        &app,
        Method::GET,
        "/ws",
        "127.0.0.1:4000",
        Some("https://evil.example"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn rebound_host_is_rejected_on_loopback() {
    let app = setup().await;
    let status = send(&app, Method::GET, "/api/runs", "evil.example:4000", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn local_client_without_origin_passes() {
    let app = setup().await;
    let status = send(&app, Method::GET, "/api/runs", "127.0.0.1:4000", None).await;
    assert_eq!(status, StatusCode::OK);
}

// Helpers

async fn setup() -> axum::Router {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");
    let api = ApiServer::new(db).listening_on("127.0.0.1:4000".parse().unwrap());
    build_router(api)
}

async fn send(
    app: &axum::Router,
    method: Method,
    path: &str,
    host: &str,
    origin: Option<&str>,
) -> StatusCode {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("host", host);
    if let Some(origin) = origin {
        builder = builder.header("origin", origin);
    }
    let request = builder.body(Body::empty()).expect("request");
    app.clone()
        .oneshot(request)
        .await
        .expect("response")
        .status()
}
