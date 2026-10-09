import { useCallback, useMemo, useRef } from 'react';
import type { Node } from '@xyflow/react';
import type { GraphNode, TaskProgress } from '../types';
import { getNodeStyle, NODE_WIDTH, NODE_HEIGHT } from './graphUtils';
import { GROUP_FRAME_PREFIX, COND_PREFIX, type GroupFrameNodeData, type ConditionGateNodeData } from './GraphNodeComponents';

const TERMINAL_TASK_STATUSES = new Set(['success', 'failed', 'cancelled', 'canceled', 'skipped']);
const FRAME_PADDING = 16;
const FRAME_HEADER_HEIGHT = 28;

interface DecoratedNodeCache {
  layoutNode: Node;
  status: string;
  progressKey: string;
  isMultiSelected: boolean;
  isSelected: boolean;
  decorated: Node;
}

interface UseDecoratedNodesParams {
  graphNodes: GraphNode[];
  layoutNodes: Node[];
  statusMap: Record<string, string>;
  progressMap: Record<string, TaskProgress | undefined>;
  nodeParams?: Record<string, Record<string, unknown>>;
  selectedSet: Set<string>;
  deferredSelectedNodeId: string | undefined;
  taskGroups: Record<string, string[]>;
  collapsedGroups: Set<string>;
  onToggleGroup?: (groupName: string) => void;
}

export function useDecoratedNodes({
  layoutNodes,
  statusMap,
  progressMap,
  nodeParams,
  selectedSet,
  deferredSelectedNodeId,
  taskGroups,
  collapsedGroups,
  onToggleGroup,
}: UseDecoratedNodesParams) {
  const decoratedNodeCacheRef = useRef<Map<string, DecoratedNodeCache>>(new Map());

  const normalizeProgress = useCallback((raw: TaskProgress | undefined) => {
    if (!raw || typeof raw !== 'object') return null;
    const current = Number(raw.current);
    const total = Number(raw.total);
    if (!Number.isFinite(current) || !Number.isFinite(total) || total <= 0) return null;
    const ratio = Math.min(1, Math.max(0, current / total));
    const percent = Math.round(ratio * 100);
    const message = typeof raw.message === 'string' ? raw.message.trim() : '';
    return { current: Math.max(0, Math.round(current)), total: Math.max(1, Math.round(total)), percent, message };
  }, []);

  const nodes = useMemo(() => {
    const previousCache = decoratedNodeCacheRef.current;
    const nextCache = new Map<string, DecoratedNodeCache>();

    const nextNodes = layoutNodes.map((node) => {
      if (node.id.startsWith(COND_PREFIX)) {
        const taskId = node.id.slice(COND_PREFIX.length);
        const conditionExpr = typeof nodeParams?.[taskId]?.condition_expr === 'string'
          ? (nodeParams[taskId].condition_expr as string) : '';
        const taskStatus = (statusMap[taskId] || '').toLowerCase();
        const gateStatus = taskStatus === 'skipped' ? 'skipped'
          : ['success', 'failed', 'running', 'dispatched', 'paused'].includes(taskStatus) ? 'success' : '';
        const baseStyle = getNodeStyle(gateStatus);
        const dashedBorder = (baseStyle.border as string | undefined)?.replace('solid', 'dashed') ?? `1.5px dashed var(--interactive-border)`;
        return {
          ...node,
          type: 'conditionGate',
          selectable: false,
          draggable: false,
          focusable: false,
          data: { ...node.data, conditionExpr } satisfies ConditionGateNodeData,
          style: { ...node.style, ...baseStyle, border: dashedBorder, borderRadius: '6px' },
        };
      }

      const nodeStatus = statusMap[node.id] || '';
      const isTerminal = TERMINAL_TASK_STATUSES.has(nodeStatus.toLowerCase());
      const progress = isTerminal ? null : normalizeProgress(progressMap[node.id]);
      const progressKey = progress ? `${progress.current}/${progress.total}:${progress.percent}:${progress.message}` : '';
      const isMultiSelected = selectedSet.has(node.id);
      const isSelected = node.id === deferredSelectedNodeId;
      const nodeLabel = String(node.data?.label ?? node.id);
      const cached = previousCache.get(node.id);

      if (
        cached && cached.layoutNode === node && cached.status === nodeStatus
        && cached.progressKey === progressKey && cached.isMultiSelected === isMultiSelected
        && cached.isSelected === isSelected
      ) {
        nextCache.set(node.id, cached);
        return cached.decorated;
      }

      const decorated: Node = {
        ...node,
        selected: isMultiSelected,
        data: {
          ...node.data,
          exportLabel: String(node.data?.exportLabel ?? node.id),
          status: nodeStatus,
          label: (
            <div className="graph-node-content">
              <div className="graph-node-title">{nodeLabel}</div>
              {progress && (
                <div className="graph-node-progress-wrap">
                  <div className="graph-node-progress-track">
                    <div className="graph-node-progress-fill" style={{ width: `${progress.percent}%` }} />
                  </div>
                  <div className="graph-node-progress-text" title={progress.message || `${progress.current}/${progress.total} (${progress.percent}%)`}>
                    {progress.message ? progress.message : `${progress.current}/${progress.total} (${progress.percent}%)`}
                  </div>
                </div>
              )}
            </div>
          ),
        },
        style: {
          ...node.style,
          ...getNodeStyle(nodeStatus),
          height: progress ? 62 : node.style?.height,
          padding: progress ? '6px 8px' : node.style?.padding,
          alignItems: progress ? 'stretch' : node.style?.alignItems,
          ...(isMultiSelected ? { boxShadow: '0 0 0 2px var(--accent-2), 0 0 16px var(--interactive-bg-tint)' } : {}),
          ...(isSelected ? { outline: '2px solid white', outlineOffset: '2px' } : {}),
        },
      };
      nextCache.set(node.id, { layoutNode: node, status: nodeStatus, progressKey, isMultiSelected, isSelected, decorated });
      return decorated;
    });

    decoratedNodeCacheRef.current = nextCache;
    return nextNodes;
  }, [deferredSelectedNodeId, layoutNodes, nodeParams, normalizeProgress, progressMap, selectedSet, statusMap]);

  const groupFrameNodes = useMemo<Node[]>(() => {
    if (!onToggleGroup || Object.keys(taskGroups).length === 0) return [];
    const layoutNodeMap = new Map(layoutNodes.map((n) => [n.id, n]));
    const frames: Node[] = [];
    for (const [groupName, taskNames] of Object.entries(taskGroups)) {
      if (collapsedGroups.has(groupName)) continue;
      const memberNodes = taskNames.map((t) => layoutNodeMap.get(t)).filter(Boolean) as Node[];
      if (memberNodes.length === 0) continue;
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const n of memberNodes) {
        const w = Number(n.style?.width ?? NODE_WIDTH);
        const h = Number(n.style?.height ?? NODE_HEIGHT);
        minX = Math.min(minX, n.position.x); minY = Math.min(minY, n.position.y);
        maxX = Math.max(maxX, n.position.x + w); maxY = Math.max(maxY, n.position.y + h);
      }
      frames.push({
        id: `${GROUP_FRAME_PREFIX}${groupName}`,
        type: 'groupFrame',
        position: { x: minX - FRAME_PADDING, y: minY - FRAME_PADDING - FRAME_HEADER_HEIGHT },
        data: { groupName, taskCount: memberNodes.length, onToggle: () => onToggleGroup(groupName) } satisfies GroupFrameNodeData,
        style: {
          width: maxX - minX + FRAME_PADDING * 2,
          height: maxY - minY + FRAME_PADDING * 2 + FRAME_HEADER_HEIGHT,
          background: 'var(--interactive-bg-glow)',
          border: '1.5px solid var(--interactive-bg-active)',
          borderRadius: 10,
          zIndex: -1,
        },
        selectable: false, draggable: false, focusable: false,
      });
    }
    return frames;
  }, [collapsedGroups, layoutNodes, onToggleGroup, taskGroups]);

  const allNodes = useMemo(() => [...groupFrameNodes, ...nodes], [groupFrameNodes, nodes]);

  return { nodes, allNodes };
}
