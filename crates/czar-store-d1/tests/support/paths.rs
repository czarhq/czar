#![allow(dead_code)]

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

pub fn harness_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("wrangler-harness")
}

pub fn full_app_dir() -> PathBuf {
    project_root().join("deploy/cloudflare/full")
}

pub fn react_dist_dir() -> PathBuf {
    project_root().join("crates/czar-web-react/dist")
}

pub fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root path should resolve")
}

pub fn require_react_dist() -> Result<()> {
    let index = react_dist_dir().join("index.html");
    if index.exists() {
        return Ok(());
    }

    bail!(
        "React UI bundle missing at {}. Run `CZAR_WEB_BASE_PATH=/ pnpm --dir crates/czar-web-react run build` first.",
        index.display()
    )
}
