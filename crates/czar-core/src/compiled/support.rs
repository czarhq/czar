use super::*;

pub(super) fn resolve_file(root: &Path, task_name: &str, file: &Path) -> CzarResult<PathBuf> {
    let resolved = if file.is_relative() {
        root.join(file)
    } else {
        file.to_path_buf()
    };
    resolved.canonicalize().map_err(|_| {
        CzarError::InvalidWorkflow(WorkflowValidationError::TaskFileNotFound {
            task: task_name.to_string(),
            declared_path: file.to_path_buf(),
            resolved_path: resolved,
            workflow_root: root.to_path_buf(),
        })
    })
}

pub(super) fn resolve_declared_path(root: &Path, file: &Path) -> PathBuf {
    if file.is_relative() {
        root.join(file)
    } else {
        file.to_path_buf()
    }
}

pub(super) fn topo_sort(tasks: &[CompiledTask]) -> CzarResult<Vec<usize>> {
    let mut indegree: Vec<usize> = vec![0; tasks.len()];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); tasks.len()];

    for (idx, task) in tasks.iter().enumerate() {
        indegree[idx] = task.depends_on.len();
        for &dep in &task.depends_on {
            dependents[dep].push(idx);
        }
    }

    let mut queue: std::collections::VecDeque<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(idx, &deg)| if deg == 0 { Some(idx) } else { None })
        .collect();

    let mut topo = Vec::with_capacity(tasks.len());
    while let Some(node) = queue.pop_front() {
        topo.push(node);
        for &child in &dependents[node] {
            let entry = &mut indegree[child];
            *entry -= 1;
            if *entry == 0 {
                queue.push_back(child);
            }
        }
    }

    if topo.len() != tasks.len() {
        return Err(CzarError::InvalidWorkflow(WorkflowValidationError::Cycle {
            task: "<unknown>".into(),
        }));
    }

    Ok(topo)
}
