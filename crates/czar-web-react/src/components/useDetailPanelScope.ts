import { useMemo } from 'react';
import type {
  GraphNode,
  RunDetail,
  TaskProgress,
  WorkflowInfo,
} from '../types';
import { normalizeRunTaskNames, taskSetSignature } from './detailPanelUtils';

interface UseDetailPanelScopeArgs {
  selectedRun: string;
  runDetail: RunDetail | null;
  workflowDetail: WorkflowInfo | null;
  workflowHistoryRuns: RunDetail[];
  workflowGridTaskNames: string[];
  runGraphNodes: GraphNode[];
  runStatusMap: Record<string, string>;
  runProgressMap: Record<string, TaskProgress | undefined>;
  workflowGraphNodes: GraphNode[];
  workflowStatusMap: Record<string, string>;
}

interface DetailPanelScope {
  selectedRunDetail: RunDetail | null;
  activeGraphNodes: GraphNode[];
  activeStatusMap: Record<string, string>;
  activeProgressMap: Record<string, TaskProgress | undefined>;
  graphLoading: boolean;
  workflowTasks: WorkflowInfo['tasks'];
  sourceYaml: string | undefined;
  flowNames: string[];
  scopedGridRuns: RunDetail[];
  scopedGridTaskNames: string[];
  showGridPanel: boolean;
  showGridLoading: boolean;
  graphEmptyMessage: string;
}

export function useDetailPanelScope({
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
}: UseDetailPanelScopeArgs): DetailPanelScope {
  const displayedRunDetail = selectedRun ? runDetail : null;
  const selectedRunDetail = selectedRun && runDetail?.run.id === selectedRun ? runDetail : null;
  const isRunView = Boolean(selectedRun);
  const graphLoading = Boolean(selectedRun && !displayedRunDetail);

  const activeGraphNodes = isRunView
    ? (displayedRunDetail ? runGraphNodes : [])
    : workflowGraphNodes;
  const activeStatusMap = isRunView
    ? (displayedRunDetail ? runStatusMap : {})
    : workflowStatusMap;
  const activeProgressMap = isRunView
    ? (displayedRunDetail ? runProgressMap : {})
    : {};

  const workflowTasks = runDetail?.workflow?.tasks ?? workflowDetail?.tasks ?? [];
  const sourceYaml = workflowDetail?.source_yaml ?? runDetail?.workflow?.source_yaml;
  const flowNames = workflowDetail?.flows ?? [];

  const selectedRunFlowName = selectedRunDetail?.run.task_selection?.flow_name?.trim() ?? '';
  const selectedRunIsAdHocSelection = Boolean(
    selectedRunDetail?.run.task_selection && selectedRunFlowName.length === 0
  );
  const selectedRunTaskNames = useMemo<string[]>(
    () => (selectedRunDetail ? normalizeRunTaskNames(selectedRunDetail) : []),
    [selectedRunDetail]
  );
  const selectedRunTaskSignature = useMemo(
    () => taskSetSignature(selectedRunTaskNames),
    [selectedRunTaskNames]
  );

  const scopedGridRuns = useMemo(() => {
    if (!selectedRunDetail) return workflowHistoryRuns;
    if (selectedRunIsAdHocSelection) return [] as RunDetail[];

    const filtered = workflowHistoryRuns.filter((run) => {
      if (selectedRunFlowName) {
        const runFlowName = run.run.task_selection?.flow_name?.trim() ?? '';
        if (runFlowName !== selectedRunFlowName) {
          return false;
        }
      }
      return taskSetSignature(normalizeRunTaskNames(run)) === selectedRunTaskSignature;
    });

    if (!filtered.some((run) => run.run.id === selectedRunDetail.run.id)) {
      return [selectedRunDetail, ...filtered];
    }
    return filtered;
  }, [
    selectedRunDetail,
    selectedRunFlowName,
    selectedRunIsAdHocSelection,
    selectedRunTaskSignature,
    workflowHistoryRuns,
  ]);

  const scopedGridTaskNames = selectedRunDetail ? selectedRunTaskNames : workflowGridTaskNames;
  const showGridPanel = !selectedRun
    || (selectedRunDetail ? !selectedRunIsAdHocSelection : true);
  const showGridLoading = Boolean(
    selectedRun
    && !selectedRunDetail
    && workflowHistoryRuns.length === 0
    && workflowGridTaskNames.length === 0
  );
  const graphEmptyMessage = selectedRun
    ? (displayedRunDetail ? 'No tasks in this run.' : 'Loading run graph...')
    : 'No workflow loaded.';

  return {
    selectedRunDetail,
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
  };
}
