use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::env::{
    EnvBinding, SecretFileBinding, collect_env_bindings, collect_secret_file_bindings,
};

pub(super) type ProcessEnvBuild = (
    HashMap<String, String>,
    Vec<SecretFileBinding>,
    Option<PathBuf>,
    Option<PathBuf>,
    Option<String>,
    Option<String>,
    Option<serde_json::Value>,
    Option<serde_json::Value>,
    Option<PathBuf>,
    Option<PathBuf>,
    Vec<String>,
    Option<String>,
);

pub(super) const CONTAINER_WORKSPACE_DIR: &str = "/workspace";

pub(super) fn build_env_vars(params: Option<serde_json::Value>) -> Result<ProcessEnvBuild> {
    let mut env_vars = HashMap::new();
    let mut secret_file_mounts = Vec::new();
    let mut task_file = None;
    let mut config_file = None;
    let mut task_module = None;
    let mut task_function = None;
    let mut custom_executor = None;
    let mut task_input = None;
    let mut runner_path = None;
    let mut python_path = None;
    let mut task_args: Vec<String> = Vec::new();
    let mut container_image = None;

    if let Some(params) = params
        && let Some(obj) = params.as_object()
    {
        for (k, v) in obj.iter() {
            match k.as_str() {
                "task_file" => {
                    if let Some(path) = v.as_str() {
                        task_file = Some(PathBuf::from(path));
                    }
                }
                "config_file" => {
                    if let Some(path) = v.as_str() {
                        config_file = Some(PathBuf::from(path));
                    }
                }
                "task_input" => {
                    task_input = Some(v.clone());
                    env_vars.insert("CZAR_INPUT_JSON".to_string(), v.to_string());
                }
                "custom_executor" => {
                    custom_executor = Some(v.clone());
                }
                "task_module" => {
                    if let Some(module) = v.as_str() {
                        task_module = Some(module.to_string());
                    }
                }
                "task_function" => {
                    if let Some(function) = v.as_str() {
                        task_function = Some(function.to_string());
                    }
                }
                "python_path" => {
                    if let Some(path) = v.as_str() {
                        python_path = Some(PathBuf::from(path));
                    }
                }
                "runner_path" => {
                    if let Some(path) = v.as_str() {
                        runner_path = Some(PathBuf::from(path));
                    }
                }
                "container_image" => {
                    if let Some(image) = v.as_str() {
                        container_image = Some(image.to_string());
                    }
                }
                "upstream" => {
                    env_vars.insert("CZAR_UPSTREAM_JSON".to_string(), v.to_string());
                }
                "env" => {
                    if v.is_object() {
                        for (env_key, env_binding) in collect_env_bindings(v)? {
                            match env_binding {
                                EnvBinding::Literal(value) => {
                                    env_vars.insert(env_key, value);
                                }
                            }
                        }
                    }
                }
                "secrets" => {
                    secret_file_mounts = collect_secret_file_bindings(v)?;
                }
                "task_args" => {
                    if let Some(arr) = v.as_array() {
                        for item in arr {
                            if let Some(s) = item.as_str() {
                                task_args.push(s.to_string());
                            }
                        }
                    }
                }
                _ => {
                    if let Some(val) = v.as_str() {
                        env_vars.insert(k.clone(), val.to_string());
                    } else if v.is_number() || v.is_boolean() {
                        env_vars.insert(k.clone(), v.to_string());
                    }
                }
            }
        }
    }

    Ok((
        env_vars,
        secret_file_mounts,
        task_file,
        config_file,
        task_module,
        task_function,
        custom_executor,
        task_input,
        runner_path,
        python_path,
        task_args,
        container_image,
    ))
}

pub(super) fn resolve_command_path(_working_dir: &str, command: &str) -> String {
    command.to_string()
}

pub(super) fn build_python_command(python_path: Option<&PathBuf>) -> Result<(Command, bool)> {
    let root = python_path.cloned().ok_or_else(|| {
        anyhow::anyhow!("python execution requires python_path resolved at compile time")
    })?;
    if !root.join("pyproject.toml").exists() {
        anyhow::bail!(
            "python execution requires a pyproject.toml at {}",
            root.display()
        );
    }
    let mut cmd = Command::new("uv");
    cmd.current_dir(&root);
    cmd.args(["run", "python"]);
    Ok((cmd, true))
}

pub(super) fn merge_pythonpath_entries(root: &std::path::Path, inherited: Option<&str>) -> String {
    let mut paths: Vec<PathBuf> = vec![root.to_path_buf()];
    if let Some(raw) = inherited {
        for path in std::env::split_paths(raw) {
            if path.as_os_str().is_empty() || paths.iter().any(|existing| existing == &path) {
                continue;
            }
            paths.push(path);
        }
    }

    std::env::join_paths(paths)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| root.to_string_lossy().to_string())
}

pub(super) fn detect_container_runtime() -> Result<String> {
    if let Ok(explicit) = std::env::var("CZAR_CONTAINER_RUNTIME") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    for candidate in ["docker", "podman"] {
        if executable_exists(candidate) {
            return Ok(candidate.to_string());
        }
    }

    anyhow::bail!(
        "container execution requires Docker or Podman on PATH, or CZAR_CONTAINER_RUNTIME set explicitly"
    );
}

pub(super) fn build_container_command(
    working_dir: &str,
    image: &str,
    command: Option<&str>,
    task_args: &[String],
    env_vars: &HashMap<String, String>,
    secret_file_mounts: &[SecretFileBinding],
) -> anyhow::Result<Command> {
    let runtime = detect_container_runtime()?;
    let mut cmd = Command::new(runtime);
    cmd.arg("run")
        .arg("--rm")
        .arg("--workdir")
        .arg(CONTAINER_WORKSPACE_DIR)
        .arg("--volume")
        .arg(format!(
            "{}:{}",
            Path::new(working_dir).display(),
            CONTAINER_WORKSPACE_DIR
        ));

    for (key, value) in env_vars {
        cmd.arg("--env").arg(format!("{key}={value}"));
    }

    for mount in secret_file_mounts {
        let host_path = resolve_host_secret_path(working_dir, &mount.path);
        let container_path = container_secret_path(&mount.path);
        cmd.arg("--volume")
            .arg(format!("{}:{}:ro", host_path.display(), container_path));
    }

    cmd.arg(image);
    if let Some(command) = command {
        cmd.arg(command);
    }
    cmd.args(task_args);
    cmd.current_dir(working_dir);
    Ok(cmd)
}

pub(super) fn rewrite_secret_paths_for_container(
    working_dir: &str,
    secret_file_mounts: &[SecretFileBinding],
    env_vars: &mut HashMap<String, String>,
) {
    for mount in secret_file_mounts {
        let key = super::secret_mounts::secret_mount_env_var(&mount.name);
        let host_path = resolve_host_secret_path(working_dir, &mount.path);
        if env_vars.get(&key).map(PathBuf::from).as_ref() == Some(&host_path) {
            env_vars.insert(key, container_secret_path(&mount.path));
        }
    }
}

fn resolve_host_secret_path(working_dir: &str, mount_path: &Path) -> PathBuf {
    if mount_path.is_absolute() {
        mount_path.to_path_buf()
    } else {
        Path::new(working_dir).join(mount_path)
    }
}

fn container_secret_path(mount_path: &Path) -> String {
    if mount_path.is_absolute() {
        mount_path.to_string_lossy().to_string()
    } else {
        format!(
            "{}/{}",
            CONTAINER_WORKSPACE_DIR,
            mount_path.to_string_lossy()
        )
    }
}

fn executable_exists(name: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_var).any(|dir| executable_in_dir(&dir, name))
}

fn executable_in_dir(dir: &Path, name: &str) -> bool {
    let candidate = dir.join(name);
    candidate.is_file()
}
