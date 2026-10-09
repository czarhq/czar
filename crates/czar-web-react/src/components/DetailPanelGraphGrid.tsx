import type { MouseEvent as ReactMouseEvent, RefObject } from 'react';
import type { GraphNode, RunDetail, TaskProgress } from '../types';
import Graph from './Graph';
import WorkflowRunsGrid from './WorkflowRunsGrid';

interface DetailPanelGraphGridProps {
  splitContainerRef: RefObject<HTMLDivElement>;
  graphPct: number;
  showGridPanel: boolean;
  activeGraphNodes: GraphNode[];
  activeStatusMap: Record<string, string>;
  activeProgressMap: Record<string, TaskProgress | undefined>;
  graphLoading: boolean;
  nodeParams?: Record<string, Record<string, unknown>>;
  selectedWorkflow: string;
  selectedRun: string;
  displayedRunId: string;
  nextScheduledAt?: string;
  graphOrientation: 'LR' | 'TB';
  onGraphOrientationChange: (orientation: 'LR' | 'TB') => void;
  requestedTaskNames: string[];
  onSelectedTaskNamesChange: (taskNames: string[]) => void;
  selectedTask: string | null;
  onTaskFocus: (taskName: string) => void;
  onSplitterMouseDown: (event: ReactMouseEvent) => void;
  showGridLoading: boolean;
  scopedGridTaskNames: string[];
  scopedGridRuns: RunDetail[];
  workflowHistoryLoading: boolean;
  onCellClick: (runId: string, taskName: string | null) => void;
  graphEmptyMessage: string;
  taskGroups: Record<string, string[]>;
  collapsedGroups: Set<string>;
  onToggleGroup: (groupName: string) => void;
  onCollapseAll: () => void;
  onExpandAll: () => void;
  colorMode?: 'dark' | 'light';
  isMobileLayout?: boolean;
}

export default function DetailPanelGraphGrid({
  splitContainerRef,
  graphPct,
  showGridPanel,
  activeGraphNodes,
  activeStatusMap,
  activeProgressMap,
  graphLoading,
  nodeParams,
  selectedWorkflow,
  selectedRun,
  displayedRunId,
  nextScheduledAt,
  graphOrientation,
  onGraphOrientationChange,
  requestedTaskNames,
  onSelectedTaskNamesChange,
  selectedTask,
  onTaskFocus,
  onSplitterMouseDown,
  showGridLoading,
  scopedGridTaskNames,
  scopedGridRuns,
  workflowHistoryLoading,
  onCellClick,
  graphEmptyMessage,
  taskGroups,
  collapsedGroups,
  onToggleGroup,
  onCollapseAll,
  onExpandAll,
  colorMode = 'dark',
  isMobileLayout = false,
}: DetailPanelGraphGridProps) {
  const groupNames = Object.keys(taskGroups).sort();
  const hasGroups = groupNames.length > 0;
  const allCollapsed = hasGroups && groupNames.every((g) => collapsedGroups.has(g));
  const allExpanded = hasGroups && groupNames.every((g) => !collapsedGroups.has(g));

  return (
    <div ref={splitContainerRef} className="workflow-split-row">
      <div className="workflow-split-graph" style={{ width: `${graphPct}%` }}>
        {hasGroups && (
          <div className="graph-group-controls">
            <span className="graph-group-label">Groups</span>
            <button
              type="button"
              className="secondary"
              onClick={onCollapseAll}
              disabled={allCollapsed}
            >
              Collapse All
            </button>
            <button
              type="button"
              className="secondary"
              onClick={onExpandAll}
              disabled={allExpanded}
            >
              Expand All
            </button>
          </div>
        )}

        <Graph
          nodes={activeGraphNodes}
          statusMap={activeStatusMap}
          progressMap={activeProgressMap}
          nodeParams={nodeParams}
          fitViewKey={displayedRunId || selectedWorkflow}
          isActive={true}
          orientation={graphOrientation}
          onOrientationChange={onGraphOrientationChange}
          downloadName={selectedRun ? `run-${selectedRun}` : `workflow-${selectedWorkflow}`}
          selectedNodeIds={requestedTaskNames}
          onSelectedNodeIdsChange={onSelectedTaskNamesChange}
          selectionEnabled={Boolean(selectedWorkflow)}
          selectedNodeId={selectedTask ?? undefined}
          onNodeClick={onTaskFocus}
          colorMode={colorMode}
          taskGroups={taskGroups}
          collapsedGroups={collapsedGroups}
          onToggleGroup={onToggleGroup}
          loading={graphLoading}
          emptyMessage={graphEmptyMessage}
          isMobileLayout={isMobileLayout}
        />
      </div>

      <div
        className="split-handle"
        onMouseDown={showGridPanel ? onSplitterMouseDown : undefined}
        style={!showGridPanel ? { visibility: 'hidden', pointerEvents: 'none' } : undefined}
      />

      <div
        className="workflow-split-grid"
        style={!showGridPanel ? { visibility: 'hidden' } : undefined}
      >
        {showGridPanel && (
          <WorkflowRunsGrid
            workflowName={selectedWorkflow}
            taskNames={scopedGridTaskNames}
            runs={scopedGridRuns}
            loading={showGridLoading || workflowHistoryLoading}
            nextScheduledAt={nextScheduledAt}
            selectedRunId={selectedRun}
            selectedTaskName={selectedTask ?? undefined}
            onCellClick={onCellClick}
          />
        )}
      </div>
    </div>
  );
}
