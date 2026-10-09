import { layoutWithDagre, type GraphOrientation } from './graphUtils';
import type { GraphNode } from '../types';

self.onmessage = (event: MessageEvent<{ nodes: GraphNode[]; orientation: GraphOrientation; seq: number }>) => {
  const { nodes, orientation, seq } = event.data;
  const result = layoutWithDagre(nodes, orientation);
  self.postMessage({ ...result, seq });
};
