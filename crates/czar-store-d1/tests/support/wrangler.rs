#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use tokio::process::{Child, Command};
use uuid::Uuid;

pub struct WranglerDev {
    child: Child,
    port: u16,
    persist_dir: PathBuf,
}

impl Default for WranglerDev {
    fn default() -> Self {
        let child = Command::new("sh")
            .arg("-lc")
            .arg("true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn inert default child");
        Self {
            child,
            port: 0,
            persist_dir: PathBuf::new(),
        }
    }
}

impl WranglerDev {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for WranglerDev {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = fs::remove_dir_all(&self.persist_dir);
    }
}

#[derive(Default)]
pub struct DevVarsGuard {
    path: PathBuf,
    original: Option<Vec<u8>>,
}

impl DevVarsGuard {
    pub fn write(path: PathBuf, contents: &str) -> Result<Self> {
        let original = fs::read(&path).ok();
        fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(Self { path, original })
    }
}

impl Drop for DevVarsGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(contents) => {
                let _ = fs::write(&self.path, contents);
            }
            None => {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

pub async fn start_wrangler_dev(
    workdir: &Path,
    config: &str,
    bind_ip: &str,
    port: u16,
) -> Result<WranglerDev> {
    let wrangler = wrangler_binary(workdir)?;
    let persist_dir = std::env::temp_dir().join(format!("czar-wrangler-{}", Uuid::new_v4()));
    fs::create_dir_all(&persist_dir)
        .with_context(|| format!("failed to create {}", persist_dir.display()))?;
    let child = Command::new(wrangler)
        .arg("dev")
        .arg("--config")
        .arg(config)
        .arg("--persist-to")
        .arg(&persist_dir)
        .arg("--ip")
        .arg(bind_ip)
        .arg("--port")
        .arg(port.to_string())
        .current_dir(workdir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to start wrangler dev in {}", workdir.display()))?;
    Ok(WranglerDev {
        child,
        port,
        persist_dir,
    })
}

fn wrangler_binary(workdir: &Path) -> Result<PathBuf> {
    let local = workdir.join("node_modules").join(".bin").join("wrangler");
    if local.exists() {
        return Ok(local);
    }

    let local_cmd = workdir
        .join("node_modules")
        .join(".bin")
        .join("wrangler.cmd");
    if local_cmd.exists() {
        return Ok(local_cmd);
    }

    bail!(
        "wrangler not found. Run `pnpm install --frozen-lockfile` in {} first",
        workdir.display()
    )
}
