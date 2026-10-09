//! How `/api` and `/ws` requests are authenticated (`czar serve --auth <mode>`):
//!
//! - `none`: no check. The default, and only allowed silently on loopback.
//! - `token`: `Authorization: Bearer <token>` (TUI, scripts), or the session
//!   cookie the web UI gets from `POST /api/auth/login`.
//!
//! A non-loopback address needs an explicit mode, so the API is never open by accident.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::bail;
use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const SESSION_COOKIE: &str = "czar_session";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    None,
    Token,
}

/// The `--auth` options, shared by `czar serve`, `czar dev` and `czar-web`.
#[derive(clap::Args, Clone, Debug, Default)]
pub struct AuthArgs {
    /// How API requests are authenticated: none or token.
    /// Defaults to `token` when --auth-token is set, else `none` (loopback only).
    #[arg(long = "auth", env = "CZAR_AUTH", value_enum)]
    pub mode: Option<AuthMode>,

    /// Token for `--auth token`: clients send `Authorization: Bearer <TOKEN>`;
    /// the web UI asks for it once.
    #[arg(long, env = "CZAR_AUTH_TOKEN", hide_env_values = true)]
    pub auth_token: Option<String>,
}

impl AuthArgs {
    /// The auth for a server listening on `addr`. Fails when the options don't
    /// fit together, or when a non-loopback address has no explicit mode.
    pub fn resolve(&self, addr: SocketAddr) -> anyhow::Result<ApiAuth> {
        let token = self
            .auth_token
            .clone()
            .filter(|token| !token.trim().is_empty());
        let mode = match (self.mode, &token) {
            (Some(mode), _) => mode,
            (None, Some(_)) => AuthMode::Token,
            (None, None) if addr.ip().is_loopback() => AuthMode::None,
            (None, None) => bail!(
                "the API would listen on {addr} with no authentication. Pick one with --auth: \
                 token (with --auth-token), or none (anyone who can reach it can run commands)"
            ),
        };
        if token.is_some() && mode != AuthMode::Token {
            bail!("--auth-token only applies to --auth token");
        }
        let auth = match mode {
            AuthMode::None => {
                if !addr.ip().is_loopback() {
                    eprintln!(
                        "WARNING: the API on {addr} has no authentication (--auth none); anyone who can reach it can run commands."
                    );
                }
                ApiAuth::none()
            }
            AuthMode::Token => match token {
                Some(token) => ApiAuth::token(token),
                None => bail!("--auth token needs --auth-token (or CZAR_AUTH_TOKEN)"),
            },
        };
        Ok(auth)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ApiAuth {
    check: Check,
}

impl ApiAuth {
    pub fn none() -> Self {
        Self {
            check: Check::Open(AuthMode::None),
        }
    }

    pub fn token(token: impl Into<String>) -> Self {
        let token: String = token.into();
        Self {
            check: Check::Token {
                session: session_value(&token).into(),
                token: token.into(),
            },
        }
    }

    pub fn mode(&self) -> AuthMode {
        match &self.check {
            Check::Open(mode) => *mode,
            Check::Token { .. } => AuthMode::Token,
        }
    }

    /// Whether the request is authenticated under this mode.
    pub async fn allows(&self, headers: &HeaderMap) -> bool {
        match &self.check {
            Check::Open(_) => true,
            Check::Token { token, session } => {
                bearer(headers).is_some_and(|given| equal(given, token))
                    || session_cookie(headers).is_some_and(|given| equal(given, session))
            }
        }
    }
}

/// Middleware for `/api` and `/ws`. The login, logout and status endpoints stay open.
pub async fn enforce(State(auth): State<Arc<ApiAuth>>, request: Request, next: Next) -> Response {
    let open = request.uri().path().starts_with("/api/auth/");
    if open || auth.allows(request.headers()).await {
        return next.run(request).await;
    }
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
        "missing or invalid credentials",
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub token: String,
}

#[derive(Serialize)]
pub struct AuthStatus {
    pub mode: AuthMode,
    pub authenticated: bool,
}

/// `POST /api/auth/login` (`--auth token`): trade the token for an HttpOnly session cookie.
pub async fn login(
    State(auth): State<Arc<ApiAuth>>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Response {
    let Check::Token { token, session } = &auth.check else {
        return (StatusCode::NOT_FOUND, "this server has no token login").into_response();
    };
    if !equal(&body.token, token) {
        return (StatusCode::UNAUTHORIZED, "invalid API token").into_response();
    }
    let secure = if is_https(&headers) { "; Secure" } else { "" };
    let cookie = format!(
        "{SESSION_COOKIE}={session}; Path=/; HttpOnly; SameSite=Strict; Max-Age={SESSION_MAX_AGE_SECS}{secure}"
    );
    set_cookie(StatusCode::NO_CONTENT, &cookie)
}

/// `POST /api/auth/logout`: drop the session cookie.
pub async fn logout() -> Response {
    let cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0");
    set_cookie(StatusCode::NO_CONTENT, &cookie)
}

/// `GET /api/auth/status`: the mode, and whether this request passes it
/// (the UI shows its login page for `token` when it doesn't).
pub async fn status(State(auth): State<Arc<ApiAuth>>, headers: HeaderMap) -> Json<AuthStatus> {
    Json(AuthStatus {
        mode: auth.mode(),
        authenticated: auth.allows(&headers).await,
    })
}

// Helpers

const SESSION_MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60;

#[derive(Clone, Debug)]
enum Check {
    Open(AuthMode),
    Token { token: Arc<str>, session: Arc<str> },
}

impl Default for Check {
    fn default() -> Self {
        Self::Open(AuthMode::None)
    }
}

/// The cookie holds a hash of the token rather than the token itself; changing
/// the token logs every browser out.
fn session_value(token: &str) -> String {
    hex::encode(Sha256::digest(format!("czar-session-v1\0{token}")))
}

fn equal(given: &str, expected: &str) -> bool {
    given.as_bytes().ct_eq(expected.as_bytes()).into()
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            (name == SESSION_COOKIE).then_some(value)
        })
}

/// Mark the cookie `Secure` when the browser talks HTTPS (directly or through a proxy).
fn is_https(headers: &HeaderMap) -> bool {
    let header_is = |name: &str, prefix: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_ascii_lowercase().starts_with(prefix))
    };
    header_is("origin", "https://") || header_is("x-forwarded-proto", "https")
}

fn set_cookie(status: StatusCode, cookie: &str) -> Response {
    let mut response = status.into_response();
    if let Ok(value) = HeaderValue::from_str(cookie) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

#[cfg(test)]
mod tests;
