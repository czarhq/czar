use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::env::SecretFileBinding;

#[derive(Debug, Default)]
pub(super) struct MountedSecretFiles {
    files: Vec<PathBuf>,
    created_dirs: Vec<PathBuf>,
}

impl Drop for MountedSecretFiles {
    fn drop(&mut self) {
        for file in &self.files {
            let _ = std::fs::remove_file(file);
        }
        for dir in self.created_dirs.iter().rev() {
            let _ = std::fs::remove_dir(dir);
        }
    }
}

pub(super) fn secret_mount_env_var(name: &str) -> String {
    let mut sanitized = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_uppercase());
        } else {
            sanitized.push('_');
        }
    }
    let trimmed = sanitized.trim_matches('_');
    let suffix = if trimmed.is_empty() {
        "SECRET"
    } else {
        trimmed
    };
    format!("CZAR_SECRET_{}_PATH", suffix)
}

fn resolve_secret_mount_path(base_dir: &Path, mount_path: &Path) -> PathBuf {
    if mount_path.is_absolute() {
        mount_path.to_path_buf()
    } else {
        base_dir.join(mount_path)
    }
}

pub(super) async fn materialize_secret_files(
    base_dir: &Path,
    secret_mounts: &[SecretFileBinding],
    env_vars: &mut HashMap<String, String>,
) -> anyhow::Result<MountedSecretFiles> {
    let mounted = MountedSecretFiles::default();

    for mount in secret_mounts {
        let target_path = resolve_secret_mount_path(base_dir, &mount.path);
        if !target_path.is_file() {
            anyhow::bail!(
                "secret file path '{}' does not exist",
                target_path.display()
            );
        }
        env_vars.insert(
            secret_mount_env_var(&mount.name),
            target_path.to_string_lossy().to_string(),
        );
    }

    Ok(mounted)
}
