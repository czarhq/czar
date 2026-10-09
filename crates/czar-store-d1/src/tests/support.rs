use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Map, Value, json};
use sqlx::sqlite::{SqlitePoolOptions, SqliteRow};
use sqlx::{Column, Row, Sqlite, ValueRef};
use tokio::net::TcpListener;

use crate::D1Config;
use crate::client::{D1QueryRequest, QueryParam};
use crate::database::D1Database;

#[derive(Default)]
pub(super) struct MockD1Server {
    base_url: String,
    token: String,
    _task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Clone)]
struct MockState {
    pool: sqlx::SqlitePool,
    token: String,
}

impl Default for MockState {
    fn default() -> Self {
        Self {
            pool: sqlx::SqlitePool::connect_lazy("sqlite::memory:")
                .expect("valid lazy sqlite pool"),
            token: String::new(),
        }
    }
}

impl MockD1Server {
    pub(super) async fn start() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await?;

        let token = "test-token".to_string();
        let state = Arc::new(MockState {
            pool,
            token: token.clone(),
        });

        let app = Router::new()
            .route(
                "/accounts/{account_id}/d1/database/{database_id}/query",
                post(handle_query),
            )
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock D1 server failed");
        });

        Ok(Self {
            base_url: format!("http://{address}"),
            token,
            _task: Some(task),
        })
    }

    pub(super) async fn database(&self) -> Result<D1Database> {
        D1Database::new(&D1Config {
            account_id: "acct".to_string(),
            database_id: "db".to_string(),
            api_token: self.token.clone(),
            base_url: Some(self.base_url.clone()),
        })
        .await
    }
}

async fn handle_query(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    Json(payload): Json<D1QueryRequest>,
) -> (StatusCode, Json<Value>) {
    let auth = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    if auth != format!("Bearer {}", state.token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "errors": [{ "message": "unauthorized" }],
                "messages": [],
                "result": []
            })),
        );
    }

    match execute_query(&state.pool, &payload).await {
        Ok((rows, changes)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "errors": [],
                "messages": [],
                "result": [{
                    "success": true,
                    "results": rows,
                    "meta": {
                        "changed_db": changes > 0,
                        "changes": changes,
                        "duration": 0.0,
                        "rows_read": rows.len(),
                        "rows_written": changes
                    }
                }]
            })),
        ),
        Err(error) => (
            StatusCode::OK,
            Json(json!({
                "success": false,
                "errors": [{ "message": error.to_string() }],
                "messages": [],
                "result": []
            })),
        ),
    }
}

async fn execute_query(
    pool: &sqlx::SqlitePool,
    payload: &D1QueryRequest,
) -> Result<(Vec<Map<String, Value>>, u64)> {
    let mut query = sqlx::query(sqlx::AssertSqlSafe(payload.sql.as_str()));
    for param in &payload.params {
        query = bind_param(query, param);
    }

    if should_fetch_rows(&payload.sql) {
        let rows = query.fetch_all(pool).await?;
        let mapped = rows.iter().map(row_to_json).collect::<Result<Vec<_>>>()?;
        Ok((mapped, 0))
    } else {
        let result = query.execute(pool).await?;
        Ok((Vec::new(), result.rows_affected()))
    }
}

fn bind_param<'q>(
    query: sqlx::query::Query<'q, Sqlite, sqlx::sqlite::SqliteArguments>,
    param: &'q QueryParam,
) -> sqlx::query::Query<'q, Sqlite, sqlx::sqlite::SqliteArguments> {
    match param {
        QueryParam::Null => query.bind(Option::<String>::None),
        QueryParam::Bool(value) => query.bind(*value),
        QueryParam::Integer(value) => query.bind(*value),
        QueryParam::Float(value) => query.bind(*value),
        QueryParam::Text(value) => query.bind(value),
    }
}

fn should_fetch_rows(sql: &str) -> bool {
    let normalized = sql.trim_start().to_uppercase();
    normalized.starts_with("SELECT")
        || normalized.starts_with("PRAGMA")
        || normalized.contains(" RETURNING ")
}

fn row_to_json(row: &SqliteRow) -> Result<Map<String, Value>> {
    let mut map = Map::new();
    for (index, column) in row.columns().iter().enumerate() {
        let value = row.try_get_raw(index)?;
        let json_value = if value.is_null() {
            Value::Null
        } else if let Ok(inner) = row.try_get::<i64, _>(index) {
            Value::Number(inner.into())
        } else if let Ok(inner) = row.try_get::<f64, _>(index) {
            Value::Number(
                serde_json::Number::from_f64(inner)
                    .context("invalid floating point value from sqlite")?,
            )
        } else if let Ok(inner) = row.try_get::<String, _>(index) {
            Value::String(inner)
        } else if let Ok(inner) = row.try_get::<Vec<u8>, _>(index) {
            Value::String(String::from_utf8_lossy(&inner).to_string())
        } else {
            Value::Null
        };
        map.insert(column.name().to_string(), json_value);
    }
    Ok(map)
}
