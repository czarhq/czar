use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn find_python_project_root(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir.join("pyproject.toml").exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

pub(crate) fn require_python_project_root(path: &Path, context: &str) -> Result<PathBuf, String> {
    find_python_project_root(path).ok_or_else(|| {
        format!(
            "{} requires a pyproject.toml ancestor for {}",
            context,
            path.display()
        )
    })
}

pub(crate) fn require_workflow_python_project_root(
    workflow_root: &Path,
    context: &str,
) -> Result<PathBuf, String> {
    if workflow_root.join("pyproject.toml").exists() {
        return Ok(workflow_root.to_path_buf());
    }

    Err(format!(
        "{} requires a pyproject.toml in workflow root {}",
        context,
        workflow_root.display()
    ))
}

pub(crate) fn require_explicit_python_project_root(
    project_root: &Path,
    context: &str,
) -> Result<PathBuf, String> {
    if project_root.join("pyproject.toml").exists() {
        return Ok(project_root.to_path_buf());
    }

    Err(format!(
        "{} requires a pyproject.toml at {}",
        context,
        project_root.display()
    ))
}

pub fn resolve_python_execution_root(
    explicit_project_root: Option<&Path>,
    task_file: Option<&Path>,
    workflow_root: Option<&Path>,
    context: &str,
) -> Result<PathBuf, String> {
    if let Some(project_root) = explicit_project_root {
        require_explicit_python_project_root(project_root, context)
    } else if let Some(path) = task_file {
        require_python_project_root(path, context)
    } else if let Some(workflow_root) = workflow_root {
        require_workflow_python_project_root(workflow_root, context)
    } else {
        Err(format!(
            "{} requires python_path, task_file, or workflow root to resolve a Python project",
            context
        ))
    }
}

pub(crate) fn uv_python_command(project_root: &Path) -> Command {
    let mut cmd = Command::new("uv");
    cmd.current_dir(project_root);
    cmd.arg("run").arg("python");
    cmd
}
