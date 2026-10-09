use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::anyhow;
use axum::{
    Router,
    extract::OriginalUri,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tower_http::services::{ServeDir, ServeFile};

use czar_core::database::Database;
#[cfg(feature = "postgres")]
use czar_store::PostgresDatabase;
#[cfg(all(not(feature = "postgres"), feature = "sqlite"))]
use czar_store::SqliteDatabase;
#[cfg(all(not(feature = "postgres"), not(feature = "sqlite"), feature = "d1"))]
use czar_store::d1::D1Database;

use crate::auth::{self, ApiAuth};
use crate::handlers;
use crate::request_guard::{self, RequestGuard};
use crate::ui_bundle::UiBundle;

pub use crate::api_helpers::{fmt_time, is_not_found};
use crate::api_realtime::websocket_handler;
use crate::api_routes::{
    cancel_run, list_runs, list_workflows, pause_run, pause_task, resume_run, resume_task,
    start_run, update_workflow_schedule, workflow_detail, workflow_task_states,
};
use crate::handlers::workflow_definitions_root;

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StateUpdate {
    RunUpdated {
        run_id: String,
        status: String,
    },
    TaskUpdated {
        run_id: String,
        task_id: String,
        status: String,
    },
}

#[derive(Clone)]
pub struct ApiServer {
    pub db: Arc<dyn Database>,
    pub broadcast_tx: broadcast::Sender<StateUpdate>,
    workflow_definitions_root: PathBuf,
    loopback_only: bool,
    allowed_origins: Vec<String>,
    auth: ApiAuth,
}

impl Default for ApiServer {
    fn default() -> Self {
        let db: Arc<dyn Database> = {
            #[cfg(feature = "postgres")]
            {
                Arc::new(PostgresDatabase::default())
            }
            #[cfg(all(not(feature = "postgres"), feature = "sqlite"))]
            {
                Arc::new(SqliteDatabase::default())
            }
            #[cfg(all(not(feature = "postgres"), not(feature = "sqlite"), feature = "d1"))]
            {
                Arc::new(D1Database::default())
            }
        };
        Self::new_with_workflow_definitions_root(db, PathBuf::from("."))
    }
}

impl ApiServer {
    pub fn new(db: Arc<dyn Database>) -> Self {
        let workflow_definitions_root =
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new_with_workflow_definitions_root(db, workflow_definitions_root)
    }

    pub fn new_with_workflow_definitions_root(
        db: Arc<dyn Database>,
        workflow_definitions_root: PathBuf,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        Self {
            db,
            broadcast_tx,
            workflow_definitions_root,
            loopback_only: false,
            allowed_origins: Vec::new(),
            auth: ApiAuth::default(),
        }
    }

    /// Record the listen address; loopback-only servers also reject foreign
    /// `Host` headers (DNS rebinding).
    pub fn listening_on(mut self, addr: SocketAddr) -> Self {
        self.loopback_only = addr.ip().is_loopback();
        self
    }

    /// Extra browser origins allowed to call the API besides loopback and
    /// same-origin, e.g. `https://czar.example.com`.
    pub fn with_allowed_origins(mut self, origins: Vec<String>) -> Self {
        self.allowed_origins = origins;
        self
    }

    /// How API requests are authenticated; see [`crate::auth`].
    pub fn with_auth(mut self, auth: ApiAuth) -> Self {
        self.auth = auth;
        self
    }

    pub fn workflow_definitions_root(&self) -> &std::path::Path {
        &self.workflow_definitions_root
    }

    pub async fn serve(self, addr: SocketAddr) -> JoinHandle<()> {
        let router = build_router(self.listening_on(addr));

        tokio::spawn(async move {
            let listener = bind_api_listener(addr).await.expect("bind address");
            axum::serve(listener, router).await.expect("server error");
        })
    }
}

pub async fn bind_api_listener(addr: SocketAddr) -> anyhow::Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind(addr).await.map_err(|err| {
        if err.kind() == std::io::ErrorKind::AddrInUse {
            anyhow!(
                "could not bind to port {}: address already in use. Is `czar serve` already running?",
                addr.port()
            )
        } else {
            anyhow!("failed to bind API server to {}: {}", addr, err)
        }
    })
}

pub fn build_router(api: ApiServer) -> Router {
    match UiBundle::resolve() {
        Ok(bundle) => build_router_with_ui_bundle(api, Some(bundle)),
        Err(_) => build_router_with_ui_bundle(api, None).fallback(ui_bundle_missing_handler),
    }
}

pub fn build_router_with_ui_bundle(api: ApiServer, ui_bundle: Option<UiBundle>) -> Router {
    let guard = Arc::new(RequestGuard::new(api.loopback_only, &api.allowed_origins));
    let api_auth = Arc::new(api.auth.clone());
    let auth_router = Router::new()
        .route("/login", axum::routing::post(auth::login))
        .route("/logout", axum::routing::post(auth::logout))
        .route("/status", get(auth::status))
        .with_state(Arc::clone(&api_auth));

    let api_router = Router::new()
        .route("/runs", get(list_runs).post(axum::routing::post(start_run)))
        .route("/runs/{id}", get(handlers::run_detail))
        .route("/runs/{id}/cancel", axum::routing::post(cancel_run))
        .route("/runs/{id}/pause", axum::routing::post(pause_run))
        .route("/runs/{id}/resume", axum::routing::post(resume_run))
        .route("/tasks/{id}/pause", axum::routing::post(pause_task))
        .route("/tasks/{id}/resume", axum::routing::post(resume_task))
        .route(
            "/workflows",
            get(list_workflows).post(handlers::create_workflow),
        )
        .route(
            "/workflows/definitions-root",
            get(workflow_definitions_root),
        )
        .route(
            "/workflows/reload",
            axum::routing::post(handlers::reload_workflow_definitions),
        )
        .route("/workers", axum::routing::post(handlers::register_worker))
        .route("/workflows/{name}", get(workflow_detail))
        .route(
            "/workflows/{name}/schedule",
            axum::routing::patch(update_workflow_schedule),
        )
        .route("/workflows/{name}/task-states", get(workflow_task_states))
        .nest("/auth", auth_router);

    let router = Router::new()
        .route("/ws", get(websocket_handler))
        .nest("/api", api_router)
        .with_state(api)
        .layer(axum::middleware::from_fn_with_state(
            api_auth,
            auth::enforce,
        ))
        .layer(axum::middleware::from_fn_with_state(
            guard,
            request_guard::enforce,
        ));

    match ui_bundle {
        Some(bundle) => {
            let index_html = bundle.index_html();
            router
                .nest_service("/assets", ServeDir::new(bundle.assets_dir()))
                .route_service("/config.js", ServeFile::new(bundle.config_js()))
                .fallback(move |original_uri: OriginalUri| {
                    let index_html = index_html.clone();
                    async move { spa_fallback_handler(original_uri, index_html).await }
                })
        }
        None => router,
    }
}

async fn ui_bundle_missing_handler() -> impl IntoResponse {
    let message = UiBundle::resolve()
        .err()
        .map(|err| err.missing_message())
        .unwrap_or_else(|| "React UI bundle could not be resolved.".to_string());
    let body = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Czar UI Missing</title></head><body><pre>{}</pre></body></html>",
        message
    );
    (StatusCode::SERVICE_UNAVAILABLE, Html(body))
}

async fn spa_fallback_handler(
    original_uri: OriginalUri,
    index_html: std::path::PathBuf,
) -> Response {
    let path = original_uri.path();
    if path == "/api"
        || path.starts_with("/api/")
        || path == "/ws"
        || path.starts_with("/ws/")
        || path == "/assets"
        || path.starts_with("/assets/")
    {
        return StatusCode::NOT_FOUND.into_response();
    }

    match tokio::fs::read_to_string(index_html).await {
        Ok(body) => Html(body).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to load React UI index.html: {err}"),
        )
            .into_response(),
    }
}
