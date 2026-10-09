import { memo, useCallback, useDeferredValue, useEffect, useMemo, useRef, useState, startTransition } from 'react';
import GraphLayoutWorker from './graphLayout.worker?worker';
import { toPng, toSvg } from 'html-to-image';
import {
  Background,
  Controls,
  ReactFlow,
  SelectionMode,
  getNodesBounds,
  getViewportForBounds,
  type Edge,
  type Node,
  type NodeMouseHandler,
  type ReactFlowInstance,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import type { GraphNode, TaskProgress } from '../types';
import GraphFitViewOnChange from './GraphFitViewOnChange';
import GraphMiniPreview from './GraphMiniPreview';
import MessageModal from './MessageModal';
import { downloadBlob, type GraphOrientation } from './graphUtils';
import { isGroupNode, getGroupNameFromNodeId } from './groupedGraphUtils';
import { GROUP_FRAME_PREFIX, COND_PREFIX, NODE_TYPES } from './GraphNodeComponents';
import { useDecoratedNodes } from './useDecoratedNodes';
import { GraphContextMenu } from './GraphContextMenu';
import { GraphNodeTooltip } from './GraphNodeTooltip';

const GRAPH_FIT_OPTIONS = { padding: 0.2, minZoom: 0.01, maxZoom: 2 } as const;
const CONTEXT_MENU_DRAG_THRESHOLD_PX = 6;

function getContextMenuNodeIdFromTarget(target: EventTarget | null) {
  if (!(target instanceof Element)) return undefined;
  return target.closest('.react-flow__node[data-id]')?.getAttribute('data-id') ?? undefined;
}

interface GraphProps {
  nodes: GraphNode[];
  statusMap?: Record<string, string>;
  progressMap?: Record<string, TaskProgress | undefined>;
  nodeParams?: Record<string, Record<string, unknown>>;
  fitViewKey?: string;
  isActive: boolean;
  orientation: GraphOrientation;
  onOrientationChange: (orientation: GraphOrientation) => void;
  downloadName?: string;
  selectedNodeIds?: string[];
  onSelectedNodeIdsChange?: (nodeIds: string[]) => void;
  selectionEnabled?: boolean;
  selectedNodeId?: string;
  onNodeClick?: (nodeId: string) => void;
  colorMode?: 'dark' | 'light';
  taskGroups?: Record<string, string[]>;
  collapsedGroups?: Set<string>;
  onToggleGroup?: (groupName: string) => void;
  loading?: boolean;
  emptyMessage?: string;
  isMobileLayout?: boolean;
}

function Graph({
  nodes: graphNodes,
  statusMap = {},
  progressMap = {},
  nodeParams,
  fitViewKey = '',
  isActive,
  orientation,
  onOrientationChange,
  downloadName = 'workflow-graph',
  selectedNodeIds = [],
  onSelectedNodeIdsChange,
  selectionEnabled = false,
  selectedNodeId,
  onNodeClick,
  colorMode = 'dark',
  taskGroups = {},
  collapsedGroups = new Set(),
  onToggleGroup,
  loading = false,
  emptyMessage = 'No graph data.',
  isMobileLayout = false,
}: GraphProps) {
  const [tooltip, setTooltip] = useState<{ nodeId: string; x: number; y: number } | null>(null);
  const [contextMenu, setContextMenu] = useState<{ nodeId?: string; x: number; y: number } | null>(null);
  const [mobileMode, setMobileMode] = useState<'pan' | 'select'>('pan');
  const mobileMenuButtonRef = useRef<HTMLButtonElement>(null);
  const [pngExportError, setPngExportError] = useState<string | null>(null);
  const [canvasResizeVersion, setCanvasResizeVersion] = useState(0);
  const canvasRef = useRef<HTMLDivElement>(null);
  const flowInstanceRef = useRef<ReactFlowInstance<Node, Edge> | null>(null);
  const lastCanvasSizeRef = useRef<{ width: number; height: number } | null>(null);
  const lastMissingRenderWarnKeyRef = useRef('');
  const rightPointerRef = useRef<{ x: number; y: number; moved: boolean; nodeId?: string } | null>(null);

  const selectedSet = useMemo(() => new Set(selectedNodeIds), [selectedNodeIds]);
  const deferredSelectedNodeId = useDeferredValue(selectedNodeId);

  const nodeToGroups = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const [groupName, nodeIds] of Object.entries(taskGroups)) {
      for (const nodeId of nodeIds) {
        const existing = map.get(nodeId) ?? [];
        existing.push(groupName);
        map.set(nodeId, existing);
      }
    }
    return map;
  }, [taskGroups]);

  const injectedGraphNodes = useMemo<GraphNode[]>(() => {
    if (!nodeParams) return graphNodes;
    const result: GraphNode[] = [];
    for (const node of graphNodes) {
      const condExpr = nodeParams[node.id]?.condition_expr;
      if (typeof condExpr === 'string' && condExpr) {
        const condId = COND_PREFIX + node.id;
        result.push({ id: condId, deps: node.deps });
        result.push({ ...node, deps: [condId] });
      } else {
        result.push(node);
      }
    }
    return result;
  }, [graphNodes, nodeParams]);

  const [layout, setLayout] = useState<{ nodes: Node[]; edges: Edge[] }>({ nodes: [], edges: [] });
  const [layoutPending, setLayoutPending] = useState(false);
  const workerRef = useRef<Worker | null>(null);
  const layoutSeqRef = useRef(0);

  useEffect(() => {
    const worker = new GraphLayoutWorker();
    workerRef.current = worker;
    worker.onmessage = (event: MessageEvent<{ nodes: Node[]; edges: Edge[]; seq: number }>) => {
      if (event.data.seq !== layoutSeqRef.current) return;
      const { nodes, edges } = event.data;
      startTransition(() => {
        setLayout({ nodes, edges });
        setLayoutPending(false);
      });
    };
    return () => { worker.terminate(); workerRef.current = null; };
  }, []);

  useEffect(() => {
    if (injectedGraphNodes.length === 0) {
      setLayout({ nodes: [], edges: [] });
      setLayoutPending(false);
      return;
    }
    const seq = ++layoutSeqRef.current;
    setLayoutPending(true);
    workerRef.current?.postMessage({ nodes: injectedGraphNodes, orientation, seq });
  }, [injectedGraphNodes, orientation]);

  // O(1) version counter — changes when topology changes, no string building.
  const graphStructureVersionRef = useRef(0);
  const graphStructureKey = useMemo(() => {
    graphStructureVersionRef.current += 1;
    return graphStructureVersionRef.current;
  }, [injectedGraphNodes]);

  const { nodes, allNodes } = useDecoratedNodes({
    graphNodes,
    layoutNodes: layout.nodes,
    statusMap,
    progressMap,
    nodeParams,
    selectedSet,
    deferredSelectedNodeId,
    taskGroups,
    collapsedGroups,
    onToggleGroup,
  });

  const edges = layout.edges;

  // Positions only change when layout changes, not when status/selection changes.
  // Using layout.nodes instead of allNodes avoids O(n) work on every status poll.
  const graphContentHeight = useMemo(() => {
    if (layout.nodes.length === 0) return 0;
    let minY = Infinity, maxY = -Infinity;
    for (const node of layout.nodes) {
      const y = node.position?.y ?? 0;
      const styleHeight = Number(node.style?.height ?? 40);
      minY = Math.min(minY, y);
      maxY = Math.max(maxY, y + (Number.isFinite(styleHeight) ? styleHeight : 40));
    }
    if (!Number.isFinite(minY) || !Number.isFinite(maxY)) return 0;
    return Math.max(0, maxY - minY);
  }, [layout.nodes]);

  const graphCanvasHeight = useMemo(
    () => Math.round(Math.max(320, Math.min(1600, graphContentHeight + 180))),
    [graphContentHeight]
  );

  // O(1) version counter — only increments when layout changes (worker response),
  // not on status/selection updates. fit-view should re-trigger on layout change only.
  const layoutVersionRef = useRef(0);
  const layoutSignature = useMemo(() => {
    layoutVersionRef.current += 1;
    return layoutVersionRef.current;
  }, [layout.nodes]);

  useEffect(() => {
    const element = canvasRef.current;
    if (!element || typeof ResizeObserver === 'undefined') return;
    const onSize = () => {
      const width = Math.round(element.clientWidth);
      const height = Math.round(element.clientHeight);
      const previous = lastCanvasSizeRef.current;
      if (previous && previous.width === width && previous.height === height) return;
      lastCanvasSizeRef.current = { width, height };
      if (width > 0 && height > 0) setCanvasResizeVersion((v) => v + 1);
    };
    onSize();
    const observer = new ResizeObserver(onSize);
    observer.observe(element);
    return () => observer.disconnect();
  }, [graphNodes.length]);

  const flowKey = `${fitViewKey}:${orientation}:${graphStructureKey}:${canvasResizeVersion}`;
  const fitTriggerKey = `${fitViewKey}:${orientation}:${layoutSignature}:${canvasResizeVersion}`;
  const flowId = `graph-${fitViewKey || downloadName || 'workflow'}`;

  useEffect(() => {
    if (graphNodes.length === 0) return;
    const timeoutId = window.setTimeout(() => {
      const element = canvasRef.current;
      if (!element) return;
      if (element.querySelectorAll('.react-flow__node').length > 0) return;
      if (lastMissingRenderWarnKeyRef.current === flowKey) return;
      lastMissingRenderWarnKeyRef.current = flowKey;
      console.warn('[graph] canvas mounted but no ReactFlow nodes rendered', {
        flowKey, graphNodeCount: graphNodes.length, layoutNodeCount: nodes.length,
        edgeCount: edges.length, canvasWidth: element.clientWidth, canvasHeight: element.clientHeight,
      });
    }, 500);
    return () => window.clearTimeout(timeoutId);
  }, [edges.length, flowKey, graphNodes.length, nodes.length]);

  useEffect(() => {
    const handleWindowMouseMove = (event: MouseEvent) => {
      const pointer = rightPointerRef.current;
      if (!pointer || (event.buttons & 2) === 0) return;
      if (pointer.moved) return;
      const dx = Math.abs(event.clientX - pointer.x);
      const dy = Math.abs(event.clientY - pointer.y);
      if (dx < CONTEXT_MENU_DRAG_THRESHOLD_PX && dy < CONTEXT_MENU_DRAG_THRESHOLD_PX) return;
      pointer.moved = true;
      setContextMenu(null);
    };

    const handleWindowMouseUp = (event: MouseEvent) => {
      if (event.button !== 2) return;
      const pointer = rightPointerRef.current;
      rightPointerRef.current = null;
      if (!pointer || pointer.moved) return;
      const canvas = canvasRef.current;
      if (!canvas) return;
      const target = event.target;
      if (target instanceof Node && !canvas.contains(target)) return;
      const nodeId = pointer.nodeId?.startsWith(COND_PREFIX) ? undefined : pointer.nodeId;
      setContextMenu({ nodeId, x: event.clientX, y: event.clientY });
    };

    const handleWindowContextMenu = (event: MouseEvent) => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const target = event.target;
      if (target instanceof Node && !canvas.contains(target)) return;
      event.preventDefault();
      event.stopPropagation();
    };

    window.addEventListener('mousemove', handleWindowMouseMove, true);
    window.addEventListener('mouseup', handleWindowMouseUp, true);
    window.addEventListener('contextmenu', handleWindowContextMenu, true);

    return () => {
      window.removeEventListener('mousemove', handleWindowMouseMove, true);
      window.removeEventListener('mouseup', handleWindowMouseUp, true);
      window.removeEventListener('contextmenu', handleWindowContextMenu, true);
    };
  }, []);

  const handleSelectionChange = useCallback((selection: { nodes: Node[]; edges: Edge[] }) => {
    if (!onSelectedNodeIdsChange) return;
    const ids = selection.nodes
      .filter((node) => !node.id.startsWith(GROUP_FRAME_PREFIX) && !node.id.startsWith(COND_PREFIX))
      .map((node) => node.id).sort();
    onSelectedNodeIdsChange(ids);
  }, [onSelectedNodeIdsChange]);

  const handleNodeClick = useCallback<NodeMouseHandler<Node>>((event, node) => {
    setContextMenu(null);
    if (node.id.startsWith(GROUP_FRAME_PREFIX) || node.id.startsWith(COND_PREFIX)) return;
    if (isGroupNode(node.id) && onToggleGroup) { onToggleGroup(getGroupNameFromNodeId(node.id)); return; }
    onNodeClick?.(node.id);
    if (!selectionEnabled || !onSelectedNodeIdsChange) return;
    const hasModifier = event.ctrlKey || event.metaKey || event.shiftKey;
    if (hasModifier) {
      const next = new Set(selectedNodeIds);
      next.has(node.id) ? next.delete(node.id) : next.add(node.id);
      onSelectedNodeIdsChange([...next].sort());
      return;
    }
    onSelectedNodeIdsChange([node.id]);
  }, [onNodeClick, onSelectedNodeIdsChange, selectedNodeIds, selectionEnabled, onToggleGroup]);

  const handlePaneClick = useCallback(() => {
    setContextMenu(null);
    if (!selectionEnabled || !onSelectedNodeIdsChange) return;
    onSelectedNodeIdsChange([]);
  }, [onSelectedNodeIdsChange, selectionEnabled]);

  const dismissContextMenu = useCallback(() => setContextMenu(null), []);

  const handleMobileMenuButtonClick = useCallback(() => {
    const btn = mobileMenuButtonRef.current;
    if (!btn) return;
    const rect = btn.getBoundingClientRect();
    setContextMenu((prev) => prev ? null : { x: rect.left, y: rect.bottom + 4 });
  }, []);

  const handleCanvasContextMenuCapture = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
  }, []);

  const handleCanvasMouseDownCapture = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    if (event.button !== 2) return;
    rightPointerRef.current = {
      x: event.clientX,
      y: event.clientY,
      moved: false,
      nodeId: getContextMenuNodeIdFromTarget(event.target),
    };
    setContextMenu(null);
  }, []);

  const handleFlowInit = useCallback((instance: ReactFlowInstance<Node, Edge>) => {
    flowInstanceRef.current = instance;
    if (!isActive) return;
    void instance.fitView({ ...GRAPH_FIT_OPTIONS, duration: 0 });
  }, [isActive]);

  if (graphNodes.length === 0) {
    return (
      <div className="graph-shell" style={{ position: 'relative' }}>
        <div
          className="graph-canvas"
          ref={canvasRef}
          style={{ height: `${graphCanvasHeight}px`, width: '100%', minWidth: 0, border: '1px solid var(--border)', borderRadius: '12px', overflow: 'hidden', background: 'var(--panel)' }}
        >
          {loading ? (
            <div className="graph-loading-shell" aria-hidden="true">
              <div className="graph-loading-node graph-loading-node--wide" />
              <div className="graph-loading-link graph-loading-link--one" />
              <div className="graph-loading-node graph-loading-node--mid" />
              <div className="graph-loading-link graph-loading-link--two" />
              <div className="graph-loading-node graph-loading-node--narrow" />
            </div>
          ) : (
            <div className="graph-empty-state">{emptyMessage}</div>
          )}
        </div>
      </div>
    );
  }

  const exportBackgroundFill = colorMode === 'light' ? '#f0f2f5' : '#0f111a';

  const getExportViewport = (exportWidth: number, exportHeight: number) => {
    const instance = flowInstanceRef.current;
    if (!instance) return null;
    const visibleNodes = instance.getNodes().filter((n) => !n.id.startsWith(GROUP_FRAME_PREFIX));
    if (visibleNodes.length === 0) return null;
    const bounds = getNodesBounds(visibleNodes);
    return getViewportForBounds(bounds, exportWidth, exportHeight, GRAPH_FIT_OPTIONS.minZoom, GRAPH_FIT_OPTIONS.maxZoom, GRAPH_FIT_OPTIONS.padding);
  };

  const getViewportElement = () => canvasRef.current?.querySelector<HTMLElement>('.react-flow__viewport') ?? null;

  const handleDownloadSvg = (): void => {
    const vp = getExportViewport(1600, 1200);
    const viewportEl = getViewportElement();
    if (!vp || !viewportEl) { setPngExportError('Graph not ready for export.'); return; }
    toSvg(viewportEl, { width: 1600, height: 1200, style: { width: '1600px', height: '1200px', transform: `translate(${vp.x}px, ${vp.y}px) scale(${vp.zoom})` }, backgroundColor: exportBackgroundFill })
      .then((dataUrl) => { const blob = new Blob([atob(dataUrl.split(',')[1])], { type: 'image/svg+xml;charset=utf-8' }); downloadBlob(blob, `${downloadName}.svg`); })
      .catch(() => setPngExportError('Failed to export graph as SVG.'));
  };

  const handleDownloadPng = (): void => {
    const vp = getExportViewport(2400, 1800);
    const viewportEl = getViewportElement();
    if (!vp || !viewportEl) { setPngExportError('Graph not ready for export.'); return; }
    toPng(viewportEl, { width: 2400, height: 1800, style: { width: '2400px', height: '1800px', transform: `translate(${vp.x}px, ${vp.y}px) scale(${vp.zoom})` }, backgroundColor: exportBackgroundFill, pixelRatio: 2 })
      .then((dataUrl) => { const a = document.createElement('a'); a.href = dataUrl; a.download = `${downloadName}.png`; a.click(); })
      .catch(() => setPngExportError('Failed to export graph as PNG.'));
  };

  return (
    <div className="graph-shell" style={{ position: 'relative' }}>
      <MessageModal
        open={Boolean(pngExportError)}
        title="Graph Export Failed"
        message={pngExportError ?? ''}
        actions={[{ label: 'Close', onClick: () => setPngExportError(null), kind: 'secondary' }]}
        onClose={() => setPngExportError(null)}
      />
      <GraphContextMenu
        contextMenu={contextMenu}
        onDismiss={dismissContextMenu}
        onToggleGroup={onToggleGroup}
        nodeToGroups={nodeToGroups}
        collapsedGroups={collapsedGroups}
        orientation={orientation}
        onOrientationChange={onOrientationChange}
        onDownloadPng={handleDownloadPng}
        onDownloadSvg={handleDownloadSvg}
      />
      <GraphNodeTooltip tooltip={tooltip} nodeParams={nodeParams} />
      <div
        className="graph-canvas"
        ref={canvasRef}
        onMouseDownCapture={handleCanvasMouseDownCapture}
        onContextMenuCapture={handleCanvasContextMenuCapture}
        style={{ height: `${graphCanvasHeight}px`, width: '100%', minWidth: 0, border: '1px solid var(--border)', borderRadius: '12px', overflow: 'hidden', background: 'var(--panel)' }}
      >
        {layoutPending && layout.nodes.length === 0 && (
          <div className="graph-loading-shell" aria-hidden="true">
            <div className="graph-loading-node graph-loading-node--wide" />
            <div className="graph-loading-link graph-loading-link--one" />
            <div className="graph-loading-node graph-loading-node--mid" />
            <div className="graph-loading-link graph-loading-link--two" />
            <div className="graph-loading-node graph-loading-node--narrow" />
          </div>
        )}
        <ReactFlow
          id={flowId}
          nodes={allNodes}
          edges={edges}
          nodeTypes={NODE_TYPES}
          minZoom={GRAPH_FIT_OPTIONS.minZoom}
          maxZoom={GRAPH_FIT_OPTIONS.maxZoom}
          nodesDraggable={false}
          nodesConnectable={false}
          elementsSelectable={selectionEnabled}
          selectionOnDrag={selectionEnabled && (!isMobileLayout || mobileMode === 'select')}
          selectionMode={SelectionMode.Partial}
          multiSelectionKeyCode={['Meta', 'Control', 'Shift']}
          panOnDrag={isMobileLayout ? (mobileMode === 'pan' || !selectionEnabled) : (selectionEnabled ? [2] : true)}
          zoomOnScroll
          zoomOnDoubleClick={false}
          onlyRenderVisibleElements
          proOptions={{ hideAttribution: true }}
          colorMode={colorMode}
          onInit={handleFlowInit}
          onSelectionChange={onSelectedNodeIdsChange ? handleSelectionChange : undefined}
          onNodeClick={handleNodeClick}
          onPaneClick={handlePaneClick}
          onNodeMouseEnter={nodeParams ? (_e, node) => {
            if (node.id.startsWith(COND_PREFIX) || node.id.startsWith(GROUP_FRAME_PREFIX)) return;
            const rect = (_e.target as HTMLElement).closest('.react-flow__node')?.getBoundingClientRect();
            setTooltip({ nodeId: node.id, x: rect ? rect.right : _e.clientX, y: rect ? rect.top + rect.height / 2 : _e.clientY });
          } : undefined}
          onNodeMouseLeave={nodeParams ? () => setTooltip(null) : undefined}
        >
          <GraphFitViewOnChange trigger={fitTriggerKey} isActive={isActive} fitOptions={GRAPH_FIT_OPTIONS} />
          <Background color="var(--border)" gap={20} />
          <Controls showInteractive={false} style={{ background: 'var(--panel-2)', border: '1px solid var(--border)' }} />
        </ReactFlow>
        <GraphMiniPreview nodes={layout.nodes} edges={layout.edges} orientation={orientation} />
      </div>
      {isMobileLayout && (
        <div className="graph-mobile-toolbar">
          {selectionEnabled && (
            <>
              <button
                type="button"
                className={`graph-mobile-tool-btn${mobileMode === 'pan' ? ' active' : ''}`}
                onClick={() => setMobileMode('pan')}
                title="Pan mode"
              >
                Pan
              </button>
              <button
                type="button"
                className={`graph-mobile-tool-btn${mobileMode === 'select' ? ' active' : ''}`}
                onClick={() => setMobileMode('select')}
                title="Select mode"
              >
                Select
              </button>
            </>
          )}
          <button
            type="button"
            ref={mobileMenuButtonRef}
            className="graph-mobile-tool-btn"
            onClick={handleMobileMenuButtonClick}
            title="Graph options"
          >
            ⋯
          </button>
        </div>
      )}
    </div>
  );
}

export default memo(Graph);
