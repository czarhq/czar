import { memo } from 'react';
import type { Edge, Node } from '@xyflow/react';

type GraphOrientation = 'LR' | 'TB';

const NODE_WIDTH = 140;
const NODE_HEIGHT = 40;

function borderColorFromStyle(border: unknown): string {
  if (typeof border !== 'string') return '#6ee7ff';
  const parts = border.trim().split(/\s+/);
  return parts[parts.length - 1] || '#6ee7ff';
}

function GraphMiniPreview({
  nodes,
  edges,
  orientation,
}: {
  nodes: Node[];
  edges: Edge[];
  orientation: GraphOrientation;
}) {
  if (nodes.length === 0) return null;

  const width = 190;
  const height = 120;
  const padding = 8;

  const positionedNodes = nodes.map((node) => {
    const style = node.style ?? {};
    return {
      id: node.id,
      x: node.position.x,
      y: node.position.y,
      width: Number(style.width ?? NODE_WIDTH),
      height: Number(style.height ?? NODE_HEIGHT),
      color: borderColorFromStyle(style.border),
    };
  });

  const minX = Math.min(...positionedNodes.map((node) => node.x));
  const minY = Math.min(...positionedNodes.map((node) => node.y));
  const maxX = Math.max(...positionedNodes.map((node) => node.x + node.width));
  const maxY = Math.max(...positionedNodes.map((node) => node.y + node.height));

  const graphWidth = Math.max(1, maxX - minX);
  const graphHeight = Math.max(1, maxY - minY);
  const sx = (width - padding * 2) / graphWidth;
  const sy = (height - padding * 2) / graphHeight;
  const scale = Math.min(sx, sy);
  const offsetX = padding + (width - padding * 2 - graphWidth * scale) / 2;
  const offsetY = padding + (height - padding * 2 - graphHeight * scale) / 2;

  const nodeMap = new Map(positionedNodes.map((node) => [node.id, node]));

  return (
    <div className="graph-minimap">
      <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`}>
        {edges.map((edge) => {
          const source = nodeMap.get(edge.source);
          const target = nodeMap.get(edge.target);
          if (!source || !target) return null;

          const sourcePoint = orientation === 'LR'
            ? { x: source.x + source.width, y: source.y + source.height / 2 }
            : { x: source.x + source.width / 2, y: source.y + source.height };
          const targetPoint = orientation === 'LR'
            ? { x: target.x, y: target.y + target.height / 2 }
            : { x: target.x + target.width / 2, y: target.y };

          const x1 = offsetX + (sourcePoint.x - minX) * scale;
          const y1 = offsetY + (sourcePoint.y - minY) * scale;
          const x2 = offsetX + (targetPoint.x - minX) * scale;
          const y2 = offsetY + (targetPoint.y - minY) * scale;

          return (
            <line
              key={edge.id}
              x1={x1}
              y1={y1}
              x2={x2}
              y2={y2}
              stroke="#6ee7ff"
              strokeOpacity="0.7"
              strokeWidth="1"
            />
          );
        })}
        {positionedNodes.map((node) => {
          const x = offsetX + (node.x - minX) * scale;
          const y = offsetY + (node.y - minY) * scale;
          const w = Math.max(3, node.width * scale);
          const h = Math.max(3, node.height * scale);
          return (
            <rect
              key={node.id}
              x={x}
              y={y}
              width={w}
              height={h}
              rx={2}
              ry={2}
              fill={node.color}
              fillOpacity="0.9"
              stroke={node.color}
              strokeWidth="0.8"
            />
          );
        })}
      </svg>
    </div>
  );
}

export default memo(GraphMiniPreview);
