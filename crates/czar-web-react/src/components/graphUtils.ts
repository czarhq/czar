import type { CSSProperties } from 'react';
import { MarkerType, Position, type Edge, type Node } from '@xyflow/react';
import dagre from '@dagrejs/dagre';
import type { GraphNode } from '../types';

export type GraphOrientation = 'LR' | 'TB';

export const NODE_WIDTH = 140;
export const NODE_HEIGHT = 40;

function uniqueCleanNames(names: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of names) {
    const name = raw.trim();
    if (!name || seen.has(name)) continue;
    seen.add(name);
    out.push(name);
  }
  return out;
}

function normalizeGraphNodes(graphNodes: GraphNode[]): GraphNode[] {
  const byId = new Map<string, GraphNode>();
  for (const node of graphNodes) {
    const id = node.id.trim();
    if (!id) continue;
    const deps = uniqueCleanNames(node.deps ?? []);
    const label = typeof node.label === 'string' ? node.label.trim() : '';
    const exportLabel = typeof node.exportLabel === 'string' ? node.exportLabel.trim() : '';
    const existing = byId.get(id);
    if (!existing) {
      byId.set(id, {
        id,
        deps,
        label: label || undefined,
        exportLabel: exportLabel || undefined,
      });
    } else {
      byId.set(id, {
        id,
        deps: uniqueCleanNames([...existing.deps, ...deps]),
        label: existing.label ?? (label || undefined),
        exportLabel: existing.exportLabel ?? (exportLabel || undefined),
      });
    }
  }
  return [...byId.values()];
}

export function getNodeStyle(status: string): CSSProperties {
  const s = status.toLowerCase();
  if (s === 'success') return { background: 'var(--node-ok-bg)', border: '2px solid var(--node-ok-color)', color: 'var(--node-ok-color)' };
  if (s === 'running' || s === 'paused' || s === 'dispatched') return { background: 'var(--node-pending-bg)', border: '2px solid var(--node-pending-color)', color: 'var(--node-pending-color)' };
  if (s === 'failed') return { background: 'var(--node-fail-bg)', border: '2px solid var(--node-fail-color)', color: 'var(--node-fail-color)' };
  if (s === 'skipped') return { background: 'var(--node-skip-bg)', border: '2px solid var(--node-skip-color)', color: 'var(--node-skip-color)' };
  return { background: 'var(--node-idle-bg)', border: '2px solid var(--node-idle-border)', color: 'var(--node-idle-color)' };
}

// Returns hardcoded hex colors for SVG export (CSS vars can't be resolved outside the browser DOM).
function getNodeExportStyle(status: string): { fill: string; stroke: string; text: string } {
  const s = status.toLowerCase();
  if (s === 'success') return { fill: '#12331b', stroke: '#a7f3d0', text: '#a7f3d0' };
  if (s === 'running' || s === 'paused' || s === 'dispatched') return { fill: '#1b1f2c', stroke: '#fbd38d', text: '#fbd38d' };
  if (s === 'failed') return { fill: '#33181c', stroke: '#ff6b6b', text: '#ff6b6b' };
  if (s === 'skipped') return { fill: '#2d1a24', stroke: '#f9a8d4', text: '#f9a8d4' };
  return { fill: '#1b1f2c', stroke: '#6ee7ff', text: '#e8ecf2' };
}

export function layoutWithDagre(graphNodes: GraphNode[], orientation: GraphOrientation): { nodes: Node[]; edges: Edge[] } {
  const normalizedNodes = normalizeGraphNodes(graphNodes);
  const g = new dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: orientation, nodesep: 50, ranksep: 80 });
  const nodeIds = new Set(normalizedNodes.map((node) => node.id));

  normalizedNodes.forEach((node) => {
    g.setNode(node.id, { width: NODE_WIDTH, height: NODE_HEIGHT });
  });

  normalizedNodes.forEach((node) => {
    (node.deps || []).forEach((dep) => {
      if (!nodeIds.has(dep)) return;
      g.setEdge(dep, node.id);
    });
  });

  dagre.layout(g);

  // Dagre places disconnected components at overlapping coordinates.
  // Detect components via BFS and stack them along the perpendicular axis.
  const allNodeIds = g.nodes();
  const adj = new Map<string, Set<string>>();
  for (const id of allNodeIds) adj.set(id, new Set());
  for (const edge of g.edges()) {
    adj.get(edge.v)?.add(edge.w);
    adj.get(edge.w)?.add(edge.v);
  }
  const visitedComp = new Set<string>();
  const components: string[][] = [];
  for (const id of allNodeIds) {
    if (visitedComp.has(id)) continue;
    const comp: string[] = [];
    const queue = [id];
    while (queue.length) {
      const curr = queue.pop()!;
      if (visitedComp.has(curr)) continue;
      visitedComp.add(curr);
      comp.push(curr);
      for (const nb of adj.get(curr) ?? []) {
        if (!visitedComp.has(nb)) queue.push(nb);
      }
    }
    components.push(comp);
  }
  if (components.length > 1) {
    // For LR layout, stack components along Y; for TB, stack along X.
    const vertical = orientation === 'LR';
    const COMP_GAP = 120;
    let cursor = 0;
    for (const comp of components) {
      let minCoord = Infinity;
      let maxCoord = -Infinity;
      for (const id of comp) {
        const pos = g.node(id);
        if (!pos) continue;
        const coord = vertical ? pos.y : pos.x;
        const half = vertical ? NODE_HEIGHT / 2 : NODE_WIDTH / 2;
        if (coord - half < minCoord) minCoord = coord - half;
        if (coord + half > maxCoord) maxCoord = coord + half;
      }
      if (!Number.isFinite(minCoord)) continue;
      const shift = cursor - minCoord;
      for (const id of comp) {
        const pos = g.node(id);
        if (!pos) continue;
        if (vertical) pos.y += shift;
        else pos.x += shift;
      }
      cursor += (maxCoord - minCoord) + COMP_GAP;
    }
  }

  const sourcePosition = orientation === 'LR' ? Position.Right : Position.Bottom;
  const targetPosition = orientation === 'LR' ? Position.Left : Position.Top;

  const nodes: Node[] = normalizedNodes.map((node, index) => {
    const pos = g.node(node.id);
    const fallbackX = orientation === 'LR' ? index * (NODE_WIDTH + 60) : 0;
    const fallbackY = orientation === 'LR' ? 0 : index * (NODE_HEIGHT + 60);
    const x = Number.isFinite(pos?.x) ? pos.x : fallbackX + NODE_WIDTH / 2;
    const y = Number.isFinite(pos?.y) ? pos.y : fallbackY + NODE_HEIGHT / 2;

    return {
      id: node.id,
      position: { x: x - NODE_WIDTH / 2, y: y - NODE_HEIGHT / 2 },
      sourcePosition,
      targetPosition,
      data: {
        label: node.label ?? node.id,
        exportLabel: node.exportLabel ?? node.label ?? node.id,
        status: '',
      },
      style: {
        ...getNodeStyle(''),
        borderRadius: '8px',
        fontSize: '12px',
        fontFamily: 'Space Grotesk, sans-serif',
        fontWeight: 600,
        width: NODE_WIDTH,
        height: NODE_HEIGHT,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: '0 8px',
      },
    };
  });

  const edgeIds = new Set<string>();
  const edges: Edge[] = [];
  normalizedNodes.forEach((node) => {
    (node.deps || []).forEach((dep) => {
      if (!nodeIds.has(dep)) return;
      const edgeId = `${dep}->${node.id}`;
      if (edgeIds.has(edgeId)) return;
      edgeIds.add(edgeId);
      edges.push({
        id: edgeId,
        source: dep,
        target: node.id,
        type: 'smoothstep',
        animated: false,
        style: { stroke: 'var(--accent-2)', strokeWidth: 2 },
        markerEnd: { type: MarkerType.ArrowClosed, color: 'var(--accent-2)' },
      });
    });
  });

  return { nodes, edges };
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

function buildEdgePath(
  source: { x: number; y: number },
  target: { x: number; y: number },
  orientation: GraphOrientation
): string {
  if (orientation === 'LR') {
    const delta = Math.max(24, Math.abs(target.x - source.x) / 2);
    return `M ${source.x} ${source.y} C ${source.x + delta} ${source.y} ${target.x - delta} ${target.y} ${target.x} ${target.y}`;
  }

  const delta = Math.max(24, Math.abs(target.y - source.y) / 2);
  return `M ${source.x} ${source.y} C ${source.x} ${source.y + delta} ${target.x} ${target.y - delta} ${target.x} ${target.y}`;
}

export function buildExportSvg(
  nodes: Node[],
  edges: Edge[],
  orientation: GraphOrientation,
  options?: {
    backgroundFill?: string;
  }
): { markup: string; width: number; height: number } {
  const backgroundFill = options?.backgroundFill ?? '#0f111a';
  if (nodes.length === 0) {
    const markup = [
      '<svg xmlns="http://www.w3.org/2000/svg" width="640" height="320" viewBox="0 0 640 320">',
      `<rect width="640" height="320" fill="${backgroundFill}"/>`,
      '<text x="320" y="160" fill="#9aa3b2" text-anchor="middle" dominant-baseline="middle" font-family="Space Grotesk, sans-serif" font-size="14">',
      'No graph data',
      '</text>',
      '</svg>',
    ].join('');
    return { markup, width: 640, height: 320 };
  }

  const positionedNodes = nodes.map((node) => {
    const style = node.style ?? {};
    const width = Number(style.width ?? NODE_WIDTH);
    const height = Number(style.height ?? NODE_HEIGHT);
    const exportColors = getNodeExportStyle(String(node.data?.status ?? ''));
    return {
      id: node.id,
      label: String(node.data?.exportLabel ?? node.id),
      x: node.position.x,
      y: node.position.y,
      width,
      height,
      fill: exportColors.fill,
      stroke: exportColors.stroke,
      text: exportColors.text,
    };
  });

  const minX = Math.min(...positionedNodes.map((node) => node.x));
  const minY = Math.min(...positionedNodes.map((node) => node.y));
  const maxX = Math.max(...positionedNodes.map((node) => node.x + node.width));
  const maxY = Math.max(...positionedNodes.map((node) => node.y + node.height));
  const padding = 24;
  const width = Math.max(1, Math.ceil(maxX - minX + padding * 2));
  const height = Math.max(1, Math.ceil(maxY - minY + padding * 2));

  const nodeMap = new Map(positionedNodes.map((node) => [node.id, node]));
  const translateX = -minX + padding;
  const translateY = -minY + padding;

  const edgeElements = edges
    .map((edge) => {
      const sourceNode = nodeMap.get(edge.source);
      const targetNode = nodeMap.get(edge.target);
      if (!sourceNode || !targetNode) return '';

      const sourcePoint = orientation === 'LR'
        ? { x: sourceNode.x + sourceNode.width, y: sourceNode.y + sourceNode.height / 2 }
        : { x: sourceNode.x + sourceNode.width / 2, y: sourceNode.y + sourceNode.height };
      const targetPoint = orientation === 'LR'
        ? { x: targetNode.x, y: targetNode.y + targetNode.height / 2 }
        : { x: targetNode.x + targetNode.width / 2, y: targetNode.y };

      const path = buildEdgePath(
        { x: sourcePoint.x + translateX, y: sourcePoint.y + translateY },
        { x: targetPoint.x + translateX, y: targetPoint.y + translateY },
        orientation
      );

      return `<path d="${path}" fill="none" stroke="#6ee7ff" stroke-width="2" marker-end="url(#arrow)"/>`;
    })
    .join('');

  const nodeElements = positionedNodes
    .map((node) => {
      const x = node.x + translateX;
      const y = node.y + translateY;
      const cx = x + node.width / 2;
      const cy = y + node.height / 2;
      return [
        `<rect x="${x}" y="${y}" width="${node.width}" height="${node.height}" rx="8" ry="8" fill="${node.fill}" stroke="${node.stroke}" stroke-width="2"/>`,
        `<text x="${cx}" y="${cy}" fill="${node.text}" text-anchor="middle" dominant-baseline="middle" font-family="Space Grotesk, sans-serif" font-weight="600" font-size="12">${escapeXml(node.label)}</text>`,
      ].join('');
    })
    .join('');

  const markup = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">`,
    '<defs>',
    '<marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">',
    '<path d="M 0 0 L 10 5 L 0 10 z" fill="#6ee7ff"/>',
    '</marker>',
    '</defs>',
    `<rect width="${width}" height="${height}" fill="${backgroundFill}"/>`,
    edgeElements,
    nodeElements,
    '</svg>',
  ].join('');

  return { markup, width, height };
}

export function downloadBlob(blob: Blob, fileName: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  URL.revokeObjectURL(url);
}
