/// Minimal TCP proxy that adds configurable per-connection latency.
///
/// Binds to a local port and forwards all traffic to the upstream address,
/// sleeping `latency_ms` before forwarding each chunk from client → upstream.
/// This simulates a DB server that is not co-located with the scheduler.
///
/// Used only by the perf-test binary — no prod code paths involved.
use anyhow::Result;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub struct DbProxy {
    pub local_addr: SocketAddr,
    _shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl Default for DbProxy {
    fn default() -> Self {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
        Self {
            local_addr: "127.0.0.1:0".parse().expect("valid socket addr"),
            _shutdown_tx: shutdown_tx,
        }
    }
}

impl DbProxy {
    /// Start a proxy that listens on `127.0.0.1:0` (random port) and forwards
    /// to `upstream` (e.g. "127.0.0.1:5432"), adding `latency_ms` of delay on
    /// each read from the client before sending to upstream.
    pub async fn start(upstream: SocketAddr, latency_ms: u64) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept = listener.accept() => {
                        match accept {
                            Ok((client, _)) => {
                                tokio::spawn(proxy_conn(client, upstream, latency_ms));
                            }
                            Err(_) => break,
                        }
                    }
                    _ = &mut shutdown_rx => break,
                }
            }
        });

        Ok(DbProxy {
            local_addr,
            _shutdown_tx: shutdown_tx,
        })
    }
}

async fn proxy_conn(mut client: TcpStream, upstream: SocketAddr, latency_ms: u64) {
    let Ok(mut server) = TcpStream::connect(upstream).await else {
        return;
    };

    let (mut cr, mut cw) = client.split();
    let (mut sr, mut sw) = server.split();

    let delay = Duration::from_millis(latency_ms);

    let client_to_server = async {
        let mut buf = [0u8; 8192];
        loop {
            match cr.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    // Apply latency before forwarding client→server data
                    tokio::time::sleep(delay).await;
                    if sw.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            }
        }
    };

    let server_to_client = async {
        let mut buf = [0u8; 8192];
        loop {
            match sr.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if cw.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            }
        }
    };

    tokio::join!(client_to_server, server_to_client);
}

/// Parse a postgres DATABASE_URL and return the server SocketAddr.
/// e.g. "postgres://user:pass@localhost:5432/dbname" → "127.0.0.1:5432"
pub fn parse_upstream_addr(database_url: &str) -> Result<SocketAddr> {
    use std::net::ToSocketAddrs;

    // Strip scheme
    let rest = database_url
        .strip_prefix("postgres://")
        .or_else(|| database_url.strip_prefix("postgresql://"))
        .ok_or_else(|| anyhow::anyhow!("unsupported database URL scheme: {database_url}"))?;

    // Skip user:pass@
    let host_part = if let Some(at_pos) = rest.find('@') {
        &rest[at_pos + 1..]
    } else {
        rest
    };

    // Strip /dbname
    let host_port = host_part
        .split('/')
        .next()
        .ok_or_else(|| anyhow::anyhow!("cannot parse host from database URL"))?;

    // host_port is "hostname:port" or just "hostname"
    let host_port_with_default = if host_port.contains(':') {
        host_port.to_string()
    } else {
        format!("{host_port}:5432")
    };

    // Resolve hostname (handles "localhost" → 127.0.0.1)
    let addr = host_port_with_default
        .to_socket_addrs()
        .map_err(|e| anyhow::anyhow!("cannot resolve '{host_port_with_default}': {e}"))?
        .next()
        .ok_or_else(|| anyhow::anyhow!("no address resolved for '{host_port_with_default}'"))?;

    Ok(addr)
}

/// Rewrite a DATABASE_URL to point to a proxy at `proxy_addr`.
/// Preserves the original credentials and database name.
pub fn rewrite_url_to_proxy(database_url: &str, proxy_addr: SocketAddr) -> String {
    // Extract credentials and dbname, replace host:port with proxy
    let rest = database_url
        .strip_prefix("postgres://")
        .or_else(|| database_url.strip_prefix("postgresql://"))
        .unwrap_or(database_url);

    let (credentials, remainder) = if let Some(at_pos) = rest.find('@') {
        (&rest[..at_pos], &rest[at_pos + 1..])
    } else {
        ("", rest)
    };

    let dbname = remainder.find('/').map(|i| &remainder[i..]).unwrap_or("");

    if credentials.is_empty() {
        format!(
            "postgres://{}:{}{}",
            proxy_addr.ip(),
            proxy_addr.port(),
            dbname
        )
    } else {
        format!(
            "postgres://{}@{}:{}{}",
            credentials,
            proxy_addr.ip(),
            proxy_addr.port(),
            dbname
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_upstream_addr() {
        let addr = parse_upstream_addr("postgres://postgres:postgres@localhost:5432/orchestrator")
            .unwrap();
        assert_eq!(addr.port(), 5432);
    }

    #[test]
    fn test_rewrite_url_to_proxy() {
        let proxy: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let result = rewrite_url_to_proxy(
            "postgres://postgres:postgres@localhost:5432/orchestrator",
            proxy,
        );
        assert_eq!(
            result,
            "postgres://postgres:postgres@127.0.0.1:12345/orchestrator"
        );
    }
}
