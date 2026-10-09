use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

fn main() -> Result<()> {
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if files.is_empty() {
        anyhow::bail!("usage: yaml-validate <file> [<file> ...]");
    }

    for path in files {
        let data = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let _: serde_yaml::Value = serde_yaml::from_str(&data)
            .with_context(|| format!("failed to parse YAML in {}", path.display()))?;
        println!("✓ {} is valid YAML", path.display());
    }

    Ok(())
}
