//! Blocks browser requests from other sites (CSRF) and, when the server only
//! listens on loopback, requests that reach it through DNS rebinding.
//!
//! Non-browser clients (CLI, TUI, curl) send no `Origin` header and are not
//! affected. Extra origins can be allowed with `czar serve --allowed-origin`.

use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use url::{Host, Url};

#[derive(Clone, Debug, Default)]
pub struct RequestGuard {
    check_host: bool,
    allowed_origins: HashSet<String>,
    allowed_hosts: HashSet<String>,
}

impl RequestGuard {
    /// `check_host` should be true when the server listens only on loopback.
    pub fn new(check_host: bool, allowed_origins: &[String]) -> Self {
        let urls: Vec<Url> = allowed_origins
            .iter()
            .filter_map(|origin| Url::parse(origin.trim()).ok())
            .collect();
        Self {
            check_host,
            allowed_origins: urls
                .iter()
                .map(|url| url.origin().ascii_serialization())
                .collect(),
            allowed_hosts: urls
                .iter()
                .filter_map(|url| url.host_str().map(str::to_ascii_lowercase))
                .collect(),
        }
    }

    pub fn check(&self, headers: &HeaderMap) -> Result<(), String> {
        let host = headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok());

        if self.check_host
            && let Some(host) = host
            && !self.host_allowed(host)
        {
            return Err(format!(
                "Host '{host}' is not allowed; allow its origin with --allowed-origin"
            ));
        }

        if let Some(origin) = headers.get(header::ORIGIN) {
            let origin = origin.to_str().unwrap_or_default();
            if !self.origin_allowed(origin, host) {
                return Err(format!(
                    "Origin '{origin}' is not allowed; allow it with --allowed-origin"
                ));
            }
        }
        Ok(())
    }
}

/// Validate an `--allowed-origin` value: an http(s) origin such as
/// `https://czar.example.com`. Anything else would be silently ignored.
pub fn parse_allowed_origin(value: &str) -> Result<String, String> {
    let url = Url::parse(value.trim()).map_err(|e| {
        format!("'{value}' is not an origin URL ({e}); use e.g. https://czar.example.com")
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(format!(
            "'{value}' must be an http(s) origin, e.g. https://czar.example.com"
        ));
    }
    Ok(url.origin().ascii_serialization())
}

pub async fn enforce(
    State(guard): State<Arc<RequestGuard>>,
    request: Request,
    next: Next,
) -> Response {
    if let Err(reason) = guard.check(request.headers()) {
        return (StatusCode::FORBIDDEN, reason).into_response();
    }
    next.run(request).await
}

// Helpers

impl RequestGuard {
    fn host_allowed(&self, host: &str) -> bool {
        let Ok(url) = Url::parse(&format!("http://{host}")) else {
            return false;
        };
        is_loopback(&url)
            || url
                .host_str()
                .is_some_and(|name| self.allowed_hosts.contains(&name.to_ascii_lowercase()))
    }

    fn origin_allowed(&self, origin: &str, host: Option<&str>) -> bool {
        let Ok(url) = Url::parse(origin) else {
            return false;
        };
        let same_origin = host.is_some_and(|host| {
            url.host_str()
                .map(|name| match url.port() {
                    Some(port) => format!("{name}:{port}"),
                    None => name.to_string(),
                })
                .is_some_and(|authority| authority.eq_ignore_ascii_case(host))
        });
        is_loopback(&url)
            || same_origin
            || self
                .allowed_origins
                .contains(&url.origin().ascii_serialization())
    }
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests;
