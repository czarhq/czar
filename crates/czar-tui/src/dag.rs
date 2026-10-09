use std::collections::{HashMap, VecDeque};

pub const NW: f64 = 16.0;
pub const NH: f64 = 3.0;
pub const HGAP: f64 = 5.0;
pub const VGAP: f64 = 1.0;
// Inner label width = NW - 2 (borders) - 2 (padding) = 12
const LABEL_W: usize = 12;

#[derive(Default)]
pub struct DagNodeLayout {
    pub x: f64,
    pub y: f64,
    /// Pre-truncated and padded to LABEL_W chars.
    pub label: String,
}

#[derive(Default)]
pub struct DagEdgeSeg {
    /// Screen-space coordinates (y=0 at top, NOT canvas-flipped).
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Default)]
pub struct DagLayout {
    pub nodes: Vec<DagNodeLayout>,
    pub edges: Vec<DagEdgeSeg>,
    pub total_w: f64,
    pub total_h: f64,
}

/// Compute a left-to-right DAG layout using Kahn's BFS (O(n+e)).
pub fn compute(names: &[String], deps: &[Vec<String>]) -> DagLayout {
    let n = names.len();
    if n == 0 {
        return DagLayout {
            nodes: vec![],
            edges: vec![],
            total_w: 0.0,
            total_h: 0.0,
        };
    }

    let name_to_idx: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, nm)| (nm.as_str(), i))
        .collect();

    // Build successor list and in-degree count.
    let mut succs: Vec<Vec<usize>> = vec![vec![]; n];
    let mut in_degree = vec![0usize; n];
    for i in 0..n {
        for dep in &deps[i] {
            if let Some(&di) = name_to_idx.get(dep.as_str()) {
                succs[di].push(i);
                in_degree[i] += 1;
            }
        }
    }

    // Kahn's BFS: assign longest-path depth (= column in LR layout).
    let mut depth = vec![0usize; n];
    let mut in_deg = in_degree.clone();
    let mut queue: VecDeque<usize> = in_deg
        .iter()
        .enumerate()
        .filter(|(_, d)| **d == 0)
        .map(|(i, _)| i)
        .collect();

    while let Some(u) = queue.pop_front() {
        for &v in &succs[u] {
            depth[v] = depth[v].max(depth[u] + 1);
            in_deg[v] -= 1;
            if in_deg[v] == 0 {
                queue.push_back(v);
            }
        }
    }

    let max_depth = *depth.iter().max().unwrap_or(&0);
    let mut layers: Vec<Vec<usize>> = vec![vec![]; max_depth + 1];
    for i in 0..n {
        layers[depth[i]].push(i);
    }

    let max_per_layer = layers.iter().map(|l| l.len()).max().unwrap_or(1);
    let total_h = max_per_layer as f64 * (NH + VGAP) - VGAP;
    let total_w = (max_depth + 1) as f64 * (NW + HGAP) - HGAP;

    let mut pos = vec![(0.0f64, 0.0f64); n];
    for (d, layer) in layers.iter().enumerate() {
        let count = layer.len();
        let lh = count as f64 * NH + (count.saturating_sub(1)) as f64 * VGAP;
        let start_y = (total_h - lh) / 2.0;
        for (j, &idx) in layer.iter().enumerate() {
            pos[idx] = (d as f64 * (NW + HGAP), start_y + j as f64 * (NH + VGAP));
        }
    }

    // Pre-truncate and pad labels.
    let nodes: Vec<DagNodeLayout> = (0..n)
        .map(|i| {
            let chars: Vec<char> = names[i].chars().collect();
            let label = if chars.len() > LABEL_W {
                let s: String = chars[..LABEL_W.saturating_sub(1)].iter().collect();
                format!("{s}…")
            } else {
                format!("{:<width$}", names[i], width = LABEL_W)
            };
            DagNodeLayout {
                x: pos[i].0,
                y: pos[i].1,
                label,
            }
        })
        .collect();

    // Pre-compute L-shaped edge segments in screen-space y (y=0 at top).
    let mut edges: Vec<DagEdgeSeg> = Vec::new();
    for i in 0..n {
        let (tx, ty) = pos[i];
        let ty_mid = ty + 1.0; // mid-row of target node box
        for dep in &deps[i] {
            if let Some(&di) = name_to_idx.get(dep.as_str()) {
                let (sx, sy) = pos[di];
                let sy_mid = sy + 1.0;
                let scx = sx + NW; // right edge of source box
                let mid_x = (scx + tx) / 2.0;
                // Horizontal from source right → midpoint
                edges.push(DagEdgeSeg {
                    x1: scx,
                    y1: sy_mid,
                    x2: mid_x,
                    y2: sy_mid,
                });
                // Vertical at midpoint
                edges.push(DagEdgeSeg {
                    x1: mid_x,
                    y1: sy_mid,
                    x2: mid_x,
                    y2: ty_mid,
                });
                // Horizontal from midpoint → target left
                edges.push(DagEdgeSeg {
                    x1: mid_x,
                    y1: ty_mid,
                    x2: tx,
                    y2: ty_mid,
                });
            }
        }
    }

    DagLayout {
        nodes,
        edges,
        total_w,
        total_h,
    }
}
