import { useEffect, useMemo } from 'react';
import { useUpdateNodeInternals, type Node } from '@xyflow/react';

interface GraphRefreshNodeInternalsProps {
  nodes: Node[];
}

export default function GraphRefreshNodeInternals({ nodes }: GraphRefreshNodeInternalsProps) {
  const updateNodeInternals = useUpdateNodeInternals();
  const nodeIds = useMemo(() => nodes.map((node) => node.id), [nodes]);

  useEffect(() => {
    if (nodeIds.length === 0) return;

    const refresh = () => {
      nodeIds.forEach((nodeId) => updateNodeInternals(nodeId));
    };

    refresh();
    const animationFrame = window.requestAnimationFrame(refresh);
    const timeoutId = window.setTimeout(refresh, 80);

    return () => {
      window.cancelAnimationFrame(animationFrame);
      window.clearTimeout(timeoutId);
    };
  }, [nodeIds, updateNodeInternals]);

  return null;
}
