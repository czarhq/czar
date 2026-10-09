use anyhow::{Context, Result};
use clap::Args;
use std::collections::{HashMap, HashSet, VecDeque};

/// Shared flow + task-selector arguments used by `run`, `trigger`, and `execute`.
#[derive(Args, Clone, Debug, Default)]
pub struct FlowSelectionArgs {
    /// Run only a named flow from the workflow definition.
    #[arg(long)]
    pub flow: Option<String>,

    /// Task selector(s), positional and repeatable.
    ///
    /// Selector syntax:
    /// - `task`       = exact task
    /// - `+task`      = task + all upstream dependencies
    /// - `task+`      = task + all downstream dependants
    /// - `+task+`     = task + full neighborhood
    /// - `2+task`     = upstream depth 2
    /// - `task+2`     = downstream depth 2
    /// - `2+task+1`   = upstream depth 2 and downstream depth 1
    #[arg(value_name = "SELECTOR", verbatim_doc_comment)]
    pub selectors: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
enum SelectionDepth {
    #[default]
    Unlimited,
    Limited(usize),
}

#[derive(Debug, Clone, Default)]
struct TaskSelectionSpec {
    seed: String,
    upstream: Option<SelectionDepth>,
    downstream: Option<SelectionDepth>,
}

#[derive(Debug, Clone, Default)]
pub struct ExpandedSelectors {
    pub requested_tasks: Vec<String>,
    pub resolved_tasks: Vec<String>,
}

pub fn expand_selectors(
    task_graph: &[(String, Vec<String>)],
    selectors: &[String],
) -> Result<ExpandedSelectors> {
    let node_order: Vec<String> = task_graph.iter().map(|(name, _)| name.clone()).collect();
    let node_set: HashSet<String> = node_order.iter().cloned().collect();
    let deps_by_node: HashMap<String, Vec<String>> = task_graph
        .iter()
        .map(|(name, deps)| (name.clone(), deps.clone()))
        .collect();

    let mut reverse_by_node: HashMap<String, Vec<String>> = HashMap::new();
    for name in &node_order {
        reverse_by_node.entry(name.clone()).or_default();
    }
    for (name, deps) in task_graph {
        for dep in deps {
            reverse_by_node
                .entry(dep.clone())
                .or_default()
                .push(name.clone());
        }
    }

    let mut requested_set = HashSet::new();
    let mut resolved_set = HashSet::new();

    for raw_spec in selectors {
        let spec = parse_task_selection_spec(raw_spec)?;
        if !node_set.contains(&spec.seed) {
            anyhow::bail!("unknown task in selection: '{}'", spec.seed);
        }
        requested_set.insert(spec.seed.clone());
        for task_name in expand_spec(&spec, &deps_by_node, &reverse_by_node) {
            resolved_set.insert(task_name);
        }
    }

    let requested_tasks = node_order
        .iter()
        .filter(|name| requested_set.contains(*name))
        .cloned()
        .collect();
    let resolved_tasks = node_order
        .iter()
        .filter(|name| resolved_set.contains(*name))
        .cloned()
        .collect();

    Ok(ExpandedSelectors {
        requested_tasks,
        resolved_tasks,
    })
}

fn parse_task_selection_spec(raw: &str) -> Result<TaskSelectionSpec> {
    let spec = raw.trim();
    if spec.is_empty() {
        anyhow::bail!("empty selector");
    }
    if spec.contains('*') || spec.contains('?') {
        anyhow::bail!(
            "invalid selector '{}': legacy wildcard syntax is no longer supported",
            spec
        );
    }

    let parts: Vec<&str> = spec.split('+').collect();
    match parts.len() {
        1 => Ok(TaskSelectionSpec {
            seed: parts[0].to_string(),
            upstream: None,
            downstream: None,
        }),
        2 => {
            let left = parts[0];
            let right = parts[1];
            if left.is_empty() && !right.is_empty() {
                return Ok(TaskSelectionSpec {
                    seed: right.to_string(),
                    upstream: Some(SelectionDepth::Unlimited),
                    downstream: None,
                });
            }
            if !left.is_empty() && right.is_empty() {
                return Ok(TaskSelectionSpec {
                    seed: left.to_string(),
                    upstream: None,
                    downstream: Some(SelectionDepth::Unlimited),
                });
            }
            if is_ascii_digits(left) && !right.is_empty() {
                return Ok(TaskSelectionSpec {
                    seed: right.to_string(),
                    upstream: Some(SelectionDepth::Limited(parse_depth(left)?)),
                    downstream: None,
                });
            }
            if !left.is_empty() && is_ascii_digits(right) {
                return Ok(TaskSelectionSpec {
                    seed: left.to_string(),
                    upstream: None,
                    downstream: Some(SelectionDepth::Limited(parse_depth(right)?)),
                });
            }
            anyhow::bail!("invalid selector '{}'", spec);
        }
        3 => {
            let up = parts[0];
            let seed = parts[1];
            let down = parts[2];
            if seed.is_empty() {
                anyhow::bail!("invalid selector '{}': missing task name", spec);
            }
            let upstream = if up.is_empty() {
                Some(SelectionDepth::Unlimited)
            } else if is_ascii_digits(up) {
                Some(SelectionDepth::Limited(parse_depth(up)?))
            } else {
                anyhow::bail!(
                    "invalid selector '{}': upstream depth must be empty or numeric",
                    spec
                );
            };
            let downstream = if down.is_empty() {
                Some(SelectionDepth::Unlimited)
            } else if is_ascii_digits(down) {
                Some(SelectionDepth::Limited(parse_depth(down)?))
            } else {
                anyhow::bail!(
                    "invalid selector '{}': downstream depth must be empty or numeric",
                    spec
                );
            };
            Ok(TaskSelectionSpec {
                seed: seed.to_string(),
                upstream,
                downstream,
            })
        }
        _ => anyhow::bail!("invalid selector '{}': too many '+' operators", spec),
    }
}

fn is_ascii_digits(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
}

fn parse_depth(value: &str) -> Result<usize> {
    value
        .parse()
        .with_context(|| format!("invalid depth '{}'", value))
}

fn expand_spec(
    spec: &TaskSelectionSpec,
    deps_by_node: &HashMap<String, Vec<String>>,
    reverse_by_node: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
    // expand_bfs always includes the seed itself, so start empty.
    let mut out = HashSet::new();
    if let Some(depth) = spec.upstream {
        let max = match depth {
            SelectionDepth::Unlimited => None,
            SelectionDepth::Limited(n) => Some(n),
        };
        out.extend(expand_bfs(&spec.seed, deps_by_node, max));
    }
    if let Some(depth) = spec.downstream {
        let max = match depth {
            SelectionDepth::Unlimited => None,
            SelectionDepth::Limited(n) => Some(n),
        };
        out.extend(expand_bfs(&spec.seed, reverse_by_node, max));
    }
    // If neither upstream nor downstream was requested, include just the seed.
    if out.is_empty() {
        out.insert(spec.seed.clone());
    }
    out
}

fn expand_bfs(
    seed: &str,
    adjacency: &HashMap<String, Vec<String>>,
    max_depth: Option<usize>,
) -> HashSet<String> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((seed.to_string(), 0usize));

    while let Some((node, depth)) = queue.pop_front() {
        if !visited.insert(node.clone()) {
            continue;
        }

        if let Some(limit) = max_depth
            && depth >= limit
        {
            continue;
        }

        if let Some(next) = adjacency.get(&node) {
            for child in next {
                queue.push_back((child.clone(), depth + 1));
            }
        }
    }

    visited
}

#[cfg(test)]
mod tests;
