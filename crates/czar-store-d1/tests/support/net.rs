#![allow(dead_code)]

use std::net::{TcpListener, UdpSocket};
use std::time::Duration;

use anyhow::{Result, bail};
use reqwest::StatusCode;
use tokio::time::{Instant, sleep};

pub fn reserve_local_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

pub fn host_ip_for_containers() -> Result<String> {
    if let Ok(value) = std::env::var("CZAR_CLOUDFLARE_HOST_IP")
        && !value.is_empty()
    {
        return Ok(value);
    }

    if let Ok(output) = std::process::Command::new("sh")
        .arg("-c")
        .arg("ip -4 addr show docker0 2>/dev/null | awk '/inet / {print $2}' | cut -d/ -f1")
        .output()
        && output.status.success()
    {
        let docker_ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !docker_ip.is_empty() {
            return Ok(docker_ip);
        }
    }

    let socket = UdpSocket::bind(("0.0.0.0", 0))?;
    socket.connect(("192.0.2.1", 80))?;
    Ok(socket.local_addr()?.ip().to_string())
}

pub async fn wait_for_http_status(
    url: &str,
    expected_status: StatusCode,
    auth: Option<(&str, &str)>,
) -> Result<()> {
    let client = reqwest::Client::new();
    // Cold local Wrangler container builds can take well over a minute.
    let deadline = Instant::now() + Duration::from_secs(180);

    loop {
        let mut request = client.get(url);
        if let Some((user, pass)) = auth {
            request = request.basic_auth(user, Some(pass));
        }

        if let Ok(response) = request.send().await
            && response.status() == expected_status
        {
            return Ok(());
        }

        if Instant::now() >= deadline {
            bail!("timed out waiting for {url} to return {expected_status}");
        }

        sleep(Duration::from_millis(250)).await;
    }
}
