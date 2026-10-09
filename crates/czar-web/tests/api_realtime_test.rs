use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use std::time::Duration;
use std::{fs, path::PathBuf};

use futures::SinkExt;
use futures::StreamExt;
use tokio::net::TcpListener;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tower::ServiceExt;
use uuid::Uuid;

use czar_store::SqliteDatabase;
use czar_web::api::{ApiServer, StateUpdate, build_router_with_ui_bundle};
use czar_web::ui_bundle::UiBundle;

fn free_local_addr() -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free tcp port");
    listener.local_addr().expect("local addr")
}

async fn request_text(app: &axum::Router, path: &str) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    (
        status,
        String::from_utf8(bytes.to_vec()).expect("utf8 body"),
    )
}

async fn setup_router() -> axum::Router {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");
    build_router_with_ui_bundle(ApiServer::new(db), Some(test_ui_bundle()))
}

fn test_ui_bundle() -> UiBundle {
    let dir = make_temp_ui_dir();
    fs::create_dir_all(dir.join("assets")).expect("create assets dir");
    fs::write(
        dir.join("index.html"),
        "<!DOCTYPE html><html><body><div id=\"root\">czar</div></body></html>",
    )
    .expect("write index");
    fs::write(dir.join("config.js"), "window.__CZAR_CONFIG__ = {};").expect("write config");
    fs::write(dir.join("assets/app.js"), "console.log('czar');").expect("write js");
    UiBundle::new(dir).expect("ui bundle")
}

fn make_temp_ui_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("czar-web-ui-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create temp ui dir");
    dir
}

#[tokio::test]
async fn test_api_routes_reachable_and_spa_routes_return_index() {
    let app = setup_router().await;

    let (api_status, _) = request_text(&app, "/api/runs").await;
    assert_eq!(api_status, StatusCode::OK);

    let (root_status, root_body) = request_text(&app, "/").await;
    assert_eq!(root_status, StatusCode::OK);
    assert!(root_body.contains("<div id=\"root\">czar</div>"));

    let (spa_status, spa_body) = request_text(&app, "/runs/example").await;
    assert_eq!(spa_status, StatusCode::OK);
    assert!(spa_body.contains("<div id=\"root\">czar</div>"));

    let (miss_status, _) = request_text(&app, "/api/does-not-exist").await;
    assert_eq!(miss_status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_websocket_receives_broadcast_updates() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");

    let api = ApiServer::new(db);
    let tx = api.broadcast_tx.clone();
    let app = build_router_with_ui_bundle(api, Some(test_ui_bundle()));

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve websocket app");
    });

    let ws_url = format!("ws://{addr}/ws");
    let (mut socket, _response) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("connect websocket");

    tx.send(StateUpdate::RunUpdated {
        run_id: "run-1".to_string(),
        status: "running".to_string(),
    })
    .expect("broadcast update");

    let incoming = timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("websocket receive timeout")
        .expect("websocket frame")
        .expect("websocket text");

    let payload = match incoming {
        Message::Text(text) => text.to_string(),
        other => panic!("expected text websocket frame, got {other:?}"),
    };
    let json: serde_json::Value = serde_json::from_str(&payload).expect("json payload");
    assert_eq!(json["type"], "run_updated");
    assert_eq!(json["run_id"], "run-1");
    assert_eq!(json["status"], "running");

    socket
        .send(Message::Text("ping".to_string().into()))
        .await
        .expect("send client message");
    socket
        .send(Message::Text("second-message".to_string().into()))
        .await
        .expect("send second client message");
    sleep(Duration::from_millis(30)).await;
    socket.close(None).await.expect("close websocket");

    server.abort();
}

#[tokio::test]
async fn test_websocket_sender_stops_after_client_disconnect() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");

    let api = ApiServer::new(db);
    let tx = api.broadcast_tx.clone();
    let app = build_router_with_ui_bundle(api, Some(test_ui_bundle()));

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve websocket app");
    });

    let ws_url = format!("ws://{addr}/ws");
    let (socket, _response) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("connect websocket");

    drop(socket);
    for idx in 0..4 {
        let _ = tx.send(StateUpdate::RunUpdated {
            run_id: format!("run-disconnected-{idx}"),
            status: "running".to_string(),
        });
        sleep(Duration::from_millis(20)).await;
    }

    let response = reqwest::get(format!("http://{addr}/api/runs"))
        .await
        .expect("server should still respond");
    assert!(response.status().is_success());

    server.abort();
}

#[tokio::test]
async fn test_api_server_serve_handles_http_requests() {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");

    let addr = free_local_addr();
    let api = ApiServer::new(db);
    let handle = api.serve(addr).await;

    let response = timeout(Duration::from_secs(3), async {
        loop {
            match reqwest::get(format!("http://{addr}/api/runs")).await {
                Ok(response) => break response,
                Err(_) => sleep(Duration::from_millis(50)).await,
            }
        }
    })
    .await
    .expect("http timeout");
    assert!(response.status().is_success());

    handle.abort();
}
