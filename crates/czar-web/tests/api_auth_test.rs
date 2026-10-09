use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

use czar_store::SqliteDatabase;
use czar_web::api::{ApiServer, build_router_with_ui_bundle};
use czar_web::auth::ApiAuth;

#[tokio::test]
async fn api_without_token_is_open() {
    let app = app(None).await;
    assert_eq!(send(&app, get("/api/runs")).await.0, StatusCode::OK);
}

#[tokio::test]
async fn api_with_token_needs_bearer_or_login_cookie() {
    let app = app(Some("s3cret")).await;

    assert_eq!(
        send(&app, get("/api/runs")).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(send(&app, get("/ws")).await.0, StatusCode::UNAUTHORIZED);

    let bearer = Request::get("/api/runs")
        .header(header::AUTHORIZATION, "Bearer s3cret")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, bearer).await.0, StatusCode::OK);

    let (status, _) = send(&app, login("wrong")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, cookie) = send(&app, login("s3cret")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let cookie = cookie.expect("login sets a cookie");
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("SameSite=Strict"), "{cookie}");
    let session = cookie.split(';').next().unwrap().to_string();

    let with_cookie = Request::get("/api/runs")
        .header(header::COOKIE, &session)
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, with_cookie).await.0, StatusCode::OK);
}

#[tokio::test]
async fn auth_status_is_always_reachable() {
    let app = app(Some("s3cret")).await;
    let response = app.clone().oneshot(get("/api/auth/status")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(status, json!({"mode": "token", "authenticated": false}));
}

// Helpers

async fn app(token: Option<&str>) -> axum::Router {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");
    let auth = token.map_or_else(ApiAuth::none, ApiAuth::token);
    let api = ApiServer::new(db).with_auth(auth);
    build_router_with_ui_bundle(api, None)
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

fn login(token: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "token": token }).to_string()))
        .unwrap()
}

/// Status and `Set-Cookie`, if any.
async fn send(app: &axum::Router, request: Request<Body>) -> (StatusCode, Option<String>) {
    let response = app.clone().oneshot(request).await.expect("response");
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .map(|value| value.to_str().unwrap().to_string());
    (response.status(), cookie)
}
