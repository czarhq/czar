import { useState, useRef, useCallback, useMemo, useTransition, useEffect, type MouseEvent as ReactMouseEvent } from 'react';
import type {
  GraphNode,
  RunDetail,
  TaskProgress,
  TriggerRunRequestOptions,
  WorkflowInfo,
  WorkflowTaskStateListItem,
} from '../types';
import DetailPanelExecutionToolbar from './DetailPanelExecutionToolbar';
import DetailPanelGraphGrid from './DetailPanelGraphGrid';
import DetailPanelOverview from './DetailPanelOverview';
import MessageModal from './MessageModal';
import RunDetailView from './RunDetailView';
import RunConfigurationBody from './RunConfigurationBody';
import TaskMaterializationPanel from './TaskMaterializationPanel';
import { useDetailPanelScope } from './useDetailPanelScope';
import { useDetailPanelSelection } from './useDetailPanelSelection';
import {
  collapseGraphGroups,
  buildGroupStatusEntries,
  isGroupNode,
  getGroupNameFromNodeId,
} from './groupedGraphUtils';

interface DetailPanelProps {
  isMobileLayout: boolean;
  selectedWorkflow: string;
  selectedRun: string;
  graphOrientation: 'LR' | 'TB';
  runDetail: RunDetail | null;
  workflowDetail: WorkflowInfo | null;
  workflowTaskStates: WorkflowTaskStateListItem[];
  workflowHistoryRuns: RunDetail[];
  workflowHistoryLoading: boolean;
  workflowGridTaskNames: string[];
  runGraphNodes: GraphNode[];
  runStatusMap: Record<string, string>;
  runProgressMap: Record<string, TaskProgress | undefined>;
  workflowGraphNodes: GraphNode[];
  workflowStatusMap: Record<string, string>;
  runTriggering: boolean;
  runDetailTab: 'details' | 'gantt';
  onGraphOrientationChange: (orientation: 'LR' | 'TB') => void;
  onGridRunSelect: (runId: string) => void;
  onRunWorkflow: (options?: TriggerRunRequestOptions) => void;
  onPauseRun: (runId: string) => void;
  onResumeRun: (runId: string) => void;
  onCancelRun: (runId: string) => void;
  onRunDetailTabLinkClick: (event: ReactMouseEvent<HTMLAnchorElement>, tab: 'details' | 'gantt') => void;
  runDetailTabHrefs: Record<'details' | 'gantt', string>;
  colorMode?: 'dark' | 'light';
  logDetails: boolean;
}

export default function DetailPanel({
  isMobileLayout,
  selectedWorkflow,
  selectedRun,
  graphOrientation,
  runDetail,
  workflowDetail,
  workflowTaskStates,
  workflowHistoryRuns,
  workflowHistoryLoading,
  workflowGridTaskNames,
  runGraphNodes,
  runStatusMap,
  runProgressMap,
  workflowGraphNodes,
  workflowStatusMap,
  runTriggering,
  runDetailTab,
  onGraphOrientationChange,
  onGridRunSelect,
  onRunWorkflow,
  onPauseRun,
  onResumeRun,
  onCancelRun,
  onRunDetailTabLinkClick,
  runDetailTabHrefs,
  colorMode = 'dark',
  logDetails,
}: DetailPanelProps) {
  const [_isPendingSelectionTransition, startSelectionTransition] = useTransition();
  const [graphPct, setGraphPct] = useState(70);
  const splitContainerRef = useRef<HTMLDivElement>(null);

  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());

  const [taskSelection, setTaskSelection] = useState<{ runId: string; taskName: string } | null>(null);
  const selectedTask = taskSelection?.runId === selectedRun ? taskSelection.taskName : null;
  const taskGroups: Record<string, string[]> = useMemo(
    () => runDetail?.workflow?.task_groups ?? workflowDetail?.task_groups ?? {},
    [runDetail, workflowDetail],
  );

  // Reset collapsed groups when the workflow changes
  useEffect(() => {
    setCollapsedGroups(new Set());
  }, [selectedWorkflow]);

  // Auto-collapse groups when the workflow has many tasks (>= 50).
  // Suppressed when ?groups=expanded is in the URL (used by benchmarks/deep-links).
  const forceExpanded = useMemo(
    () => new URLSearchParams(window.location.search).get('groups') === 'expanded',
    [],
  );
  const taskCount = useMemo(
    () => (workflowDetail?.tasks ?? runDetail?.workflow?.tasks ?? []).length,
    [workflowDetail, runDetail],
  );
  const autoCollapsedRef = useRef<string>('');
  useEffect(() => {
    if (forceExpanded) return;
    const groupNames = Object.keys(taskGroups);
    if (groupNames.length === 0) return;
    const key = `${selectedWorkflow}:${taskCount}`;
    if (autoCollapsedRef.current === key) return;
    autoCollapsedRef.current = key;
    if (taskCount >= 50) {
      setCollapsedGroups(new Set(groupNames));
    }
  }, [taskGroups, taskCount, selectedWorkflow, forceExpanded]);

  const {
    activeGraphNodes,
    activeStatusMap,
    activeProgressMap,
    graphLoading,
    workflowTasks,
    sourceYaml,
    flowNames,
    scopedGridRuns,
    scopedGridTaskNames,
    showGridPanel,
    showGridLoading,
    graphEmptyMessage,
  } = useDetailPanelScope({
    selectedRun,
    runDetail,
    workflowDetail,
    workflowHistoryRuns,
    workflowGridTaskNames,
    runGraphNodes,
    runStatusMap,
    runProgressMap,
    workflowGraphNodes,
    workflowStatusMap,
  });
  // Apply group collapsing to the graph nodes
  const displayGraphNodes = useMemo(
    () => collapseGraphGroups(activeGraphNodes, taskGroups, collapsedGroups),
    [activeGraphNodes, taskGroups, collapsedGroups],
  );

  const displayStatusMap = useMemo(() => {
    const groupEntries = buildGroupStatusEntries(activeGraphNodes, activeStatusMap, taskGroups, collapsedGroups);
    return collapsedGroups.size === 0 ? activeStatusMap : { ...activeStatusMap, ...groupEntries };
  }, [activeGraphNodes, activeStatusMap, taskGroups, collapsedGroups]);

  const handleToggleGroup = useCallback((groupName: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupName)) {
        next.delete(groupName);
      } else {
        next.add(groupName);
      }
      return next;
    });
  }, []);

  const handleCollapseAll = useCallback(() => {
    setCollapsedGroups(new Set(Object.keys(taskGroups)));
  }, [taskGroups]);

  const handleExpandAll = useCallback(() => {
    setCollapsedGroups(new Set());
  }, []);

  const {
    nodeOrder,
    selectedTaskNames,
    setSelectedTaskNames,
    selectedGroupNames,
    selectionMode,
    setSelectionMode,
    selectedFlowName,
    setSelectedFlowName,
    selectionStateFilter,
    setSelectionStateFilter,
    runDialogOpen,
    setRunDialogOpen,
    runDialogMode,
    setRunDialogMode,
    selectionRunError,
    setSelectionRunError,
    taskGroupEntries,
    requestedTaskNames,
    previewResolvedTaskNames,
    materializationRows,
    selectionStateOptions,
    stateFilteredTaskNames,
    exactModeUnmaterializedDependencyCount,
    handleToggleTaskGroup,
    handleExpandSelection,
    handleUseStateFilter,
    handleAddStateFilter,
    handleUseResolved,
  } = useDetailPanelSelection({
    selectedWorkflow,
    isMobileLayout,
    workflowDetail,
    workflowTaskStates,
    activeGraphNodes,
  });

  const handleSplitterMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const container = splitContainerRef.current;
    if (!container) return;

    const onMouseMove = (mv: MouseEvent) => {
      const rect = container.getBoundingClientRect();
      const pct = Math.min(85, Math.max(15, ((mv.clientX - rect.left) / rect.width) * 100));
      setGraphPct(pct);
    };
    const onMouseUp = () => {
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    };
    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  }, []);

  const handleCellClick = useCallback((runId: string, taskName: string | null) => {
    onGridRunSelect(runId);
    startSelectionTransition(() => {
      setTaskSelection(taskName ? { runId, taskName } : null);
    });
  }, [onGridRunSelect, startSelectionTransition]);

  const handleTaskFocus = useCallback((taskName: string | null) => {
    // Clicking a collapsed group node expands it instead of selecting a task
    if (taskName && isGroupNode(taskName)) {
      handleToggleGroup(getGroupNameFromNodeId(taskName));
      return;
    }
    const next = taskName && taskName !== selectedTask ? taskName : null;
    startSelectionTransition(() => {
      setTaskSelection(next ? { runId: selectedRun, taskName: next } : null);
    });
    if (selectedRun) {
      setSelectedTaskNames(next ? [next] : []);
    }
  }, [selectedRun, selectedTask, startSelectionTransition, handleToggleGroup]);

  const handleGraphTaskNamesChange = useCallback((taskNames: string[]) => {
    // Filter out group node ids from multi-selection
    const realTaskNames = taskNames.filter((id) => !isGroupNode(id));
    setSelectedTaskNames(realTaskNames);
    if (!selectedRun) return;
    startSelectionTransition(() => {
      if (realTaskNames.length === 1) {
        setTaskSelection({ runId: selectedRun, taskName: realTaskNames[0] });
      } else if (realTaskNames.length === 0) {
        setTaskSelection(null);
      }
    });
  }, [selectedRun, startSelectionTransition]);

  const handleRunAll = () => {
    const options: TriggerRunRequestOptions = {};
    if (selectedFlowName) {
      options.flow = selectedFlowName;
    }
    onRunWorkflow(Object.keys(options).length > 0 ? options : undefined);
  };

  const handleRunSelection = () => {
    if (requestedTaskNames.length === 0 && selectedGroupNames.length === 0) {
      setSelectionRunError('Select at least one task or group first.');
      return;
    }
    const options: TriggerRunRequestOptions = {
      selection: {
        task_names: selectedTaskNames,
        group_names: selectedGroupNames,
        mode: selectionMode,
      },
    };
    if (selectedFlowName) {
      options.flow = selectedFlowName;
    }
    onRunWorkflow(options);
  };

  const hasSelection = requestedTaskNames.length > 0 || selectedGroupNames.length > 0;
  const canSubmitRun = !runTriggering
    && (runDialogMode === 'all' || hasSelection);

  const handleOpenRunDialog = () => {
    setRunDialogMode(hasSelection ? 'selection' : 'all');
    setRunDialogOpen(true);
  };

  const handleCloseRunDialog = () => {
    setRunDialogOpen(false);
  };

  const handleSubmitRun = () => {
    if (runDialogMode === 'selection') {
      handleRunSelection();
    } else {
      handleRunAll();
    }
    handleCloseRunDialog();
  };

  const selectedTaskDef = selectedTask ? workflowTasks.find((task) => task.name === selectedTask) : null;
  const nodeParams = useMemo<Record<string, Record<string, unknown>> | undefined>(() =>
    workflowTasks.length > 0
      ? Object.fromEntries(
          workflowTasks
            .filter((task) => task.params && Object.keys(task.params).length > 0)
            .map((task) => [task.name, task.params as Record<string, unknown>])
        )
      : undefined,
    [workflowTasks]
  );

  return (
    <section id="detail">
      <MessageModal
        open={Boolean(selectionRunError)}
        title="Run Selection"
        message={selectionRunError ?? ''}
        actions={[{ label: 'Close', onClick: () => setSelectionRunError(null), kind: 'secondary' }]}
        onClose={() => setSelectionRunError(null)}
      />

      <div className="panel" style={{ overflowY: 'scroll', scrollbarGutter: 'stable' }}>
        <DetailPanelOverview
          selectedWorkflow={selectedWorkflow}
          selectedRun={selectedRun}
          runDetail={runDetail}
          workflowDetail={workflowDetail}
          sourceYaml={sourceYaml}
          onPauseRun={onPauseRun}
          onResumeRun={onResumeRun}
          onCancelRun={onCancelRun}
        />
        {!selectedWorkflow && (
          <div
            className={`execution-toolbar${isMobileLayout ? ' execution-toolbar--compact' : ''}`}
            aria-hidden="true"
            style={{ visibility: 'hidden' }}
          >
            <div className="execution-toolbar-main">
              <button className="execution-run-all" disabled>&nbsp;</button>
            </div>
          </div>
        )}
        {selectedWorkflow && (
          <DetailPanelExecutionToolbar
            isMobileLayout={isMobileLayout}
            runTriggering={runTriggering}
            requestedCount={requestedTaskNames.length}
            resolvedCount={previewResolvedTaskNames.length}
            hasSelection={hasSelection}
            runDialogOpen={runDialogOpen}
            runDialogMode={runDialogMode}
            canSubmitRun={canSubmitRun}
            onOpenRunDialog={handleOpenRunDialog}
            onCloseRunDialog={handleCloseRunDialog}
            onRunDialogModeChange={setRunDialogMode}
            onRunAll={handleRunAll}
            onRunSelection={handleRunSelection}
            onSubmitRun={handleSubmitRun}
            renderConfigurationBody={() => (
              <RunConfigurationBody
                selectionMode={selectionMode}
                onSelectionModeChange={setSelectionMode}
                flowNames={flowNames}
                selectedFlowName={selectedFlowName}
                onSelectedFlowNameChange={setSelectedFlowName}
                selectionStateFilter={selectionStateFilter}
                selectionStateOptions={selectionStateOptions}
                onSelectionStateFilterChange={setSelectionStateFilter}
                requestedTaskNames={requestedTaskNames}
                previewResolvedTaskNames={previewResolvedTaskNames}
                stateFilteredTaskNames={stateFilteredTaskNames}
                onSelectAll={() => setSelectedTaskNames(nodeOrder)}
                onClear={() => setSelectedTaskNames([])}
                onUseStateFilter={handleUseStateFilter}
                onAddStateFilter={handleAddStateFilter}
                onExpandSelection={handleExpandSelection}
                onUseResolved={handleUseResolved}
                taskGroupEntries={taskGroupEntries}
                selectedGroupNames={selectedGroupNames}
                onToggleTaskGroup={handleToggleTaskGroup}
                exactModeUnmaterializedDependencyCount={exactModeUnmaterializedDependencyCount}
                onTaskFocus={handleTaskFocus}
              />
            )}
          />
        )}

        <div className="tab-panel active">
          <DetailPanelGraphGrid
            splitContainerRef={splitContainerRef}
            graphPct={graphPct}
            showGridPanel={showGridPanel}
            activeGraphNodes={displayGraphNodes}
            activeStatusMap={displayStatusMap}
            activeProgressMap={activeProgressMap}
            graphLoading={graphLoading}
            nodeParams={nodeParams}
            selectedWorkflow={selectedWorkflow}
            selectedRun={selectedRun}
            displayedRunId={runDetail?.run.id ?? ''}
            nextScheduledAt={workflowDetail?.next_scheduled_at ?? runDetail?.workflow?.next_scheduled_at}
            graphOrientation={graphOrientation}
            onGraphOrientationChange={onGraphOrientationChange}
            requestedTaskNames={requestedTaskNames}
            onSelectedTaskNamesChange={handleGraphTaskNamesChange}
            selectedTask={selectedTask}
            onTaskFocus={handleTaskFocus}
            onSplitterMouseDown={handleSplitterMouseDown}
            showGridLoading={showGridLoading}
            scopedGridTaskNames={scopedGridTaskNames}
            scopedGridRuns={scopedGridRuns}
            workflowHistoryLoading={workflowHistoryLoading}
            onCellClick={handleCellClick}
            graphEmptyMessage={graphEmptyMessage}
            taskGroups={taskGroups}
            collapsedGroups={collapsedGroups}
            onToggleGroup={handleToggleGroup}
            onCollapseAll={handleCollapseAll}
            onExpandAll={handleExpandAll}
            colorMode={colorMode}
            isMobileLayout={isMobileLayout}
          />

          <div className="grid-run-detail">
            <div className="grid-run-detail-body">
              {!selectedRun ? (
                materializationRows.length > 0 ? (
                  <TaskMaterializationPanel
                    rows={materializationRows}
                    onTaskFocus={handleTaskFocus}
                  />
                ) : (
                  <p className="grid-run-detail-placeholder">No persisted task materialization state yet.</p>
                )
              ) : !runDetail ? (
                <RunDetailView
                  runDetail={null}
                  loading={true}
                  activeTab={runDetailTab}
                  selectedTask={selectedTask}
                  selectedTaskDef={selectedTaskDef ?? null}
                  onTaskSelect={handleTaskFocus}
                  onTabLinkClick={onRunDetailTabLinkClick}
                  tabHrefs={runDetailTabHrefs}
                  logDetails={logDetails}
                />
              ) : (
                <RunDetailView
                  runDetail={runDetail}
                  loading={false}
                  activeTab={runDetailTab}
                  selectedTask={selectedTask}
                  selectedTaskDef={selectedTaskDef ?? null}
                  onTaskSelect={handleTaskFocus}
                  onTabLinkClick={onRunDetailTabLinkClick}
                  tabHrefs={runDetailTabHrefs}
                  logDetails={logDetails}
                />
              )}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
