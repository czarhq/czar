import { useEffect, useMemo, useState } from 'react';
import type {
  GraphNode,
  TaskSelectionMode,
  WorkflowInfo,
  WorkflowTaskStateListItem,
} from '../types';
import type { TaskMaterializationRow } from './TaskMaterializationPanel';
import { resolveSelection, unique } from './selectionUtils';

interface UseDetailPanelSelectionArgs {
  selectedWorkflow: string;
  isMobileLayout: boolean;
  workflowDetail: WorkflowInfo | null;
  workflowTaskStates: WorkflowTaskStateListItem[];
  activeGraphNodes: GraphNode[];
}

interface DetailPanelSelectionState {
  nodeOrder: string[];
  depsByNode: Map<string, string[]>;
  reverseByNode: Map<string, string[]>;
  selectedTaskNames: string[];
  setSelectedTaskNames: React.Dispatch<React.SetStateAction<string[]>>;
  selectedGroupNames: string[];
  selectionMode: TaskSelectionMode;
  setSelectionMode: React.Dispatch<React.SetStateAction<TaskSelectionMode>>;
  selectedFlowName: string;
  setSelectedFlowName: React.Dispatch<React.SetStateAction<string>>;
  selectionStateFilter: string;
  setSelectionStateFilter: React.Dispatch<React.SetStateAction<string>>;
  runDialogOpen: boolean;
  setRunDialogOpen: React.Dispatch<React.SetStateAction<boolean>>;
  runDialogMode: 'all' | 'selection';
  setRunDialogMode: React.Dispatch<React.SetStateAction<'all' | 'selection'>>;
  selectionRunError: string | null;
  setSelectionRunError: React.Dispatch<React.SetStateAction<string | null>>;
  taskGroupEntries: Array<[string, string[]]>;
  requestedTaskNames: string[];
  previewResolvedTaskNames: string[];
  materializationRows: TaskMaterializationRow[];
  selectionStateOptions: string[];
  stateFilteredTaskNames: string[];
  exactModeUnmaterializedDependencyCount: number;
  handleToggleTaskGroup: (groupName: string) => void;
  handleExpandSelection: (mode: Exclude<TaskSelectionMode, 'exact'>) => void;
  handleUseStateFilter: () => void;
  handleAddStateFilter: () => void;
  handleUseResolved: () => void;
}

export function useDetailPanelSelection({
  selectedWorkflow,
  isMobileLayout,
  workflowDetail,
  workflowTaskStates,
  activeGraphNodes,
}: UseDetailPanelSelectionArgs): DetailPanelSelectionState {
  const [selectedTaskNames, setSelectedTaskNames] = useState<string[]>([]);
  const [selectedGroupNames, setSelectedGroupNames] = useState<string[]>([]);
  const [selectionMode, setSelectionMode] = useState<TaskSelectionMode>('exact');
  const [selectedFlowName, setSelectedFlowName] = useState('');
  const [selectionStateFilter, setSelectionStateFilter] = useState('all');
  const [runDialogOpen, setRunDialogOpen] = useState(false);
  const [runDialogMode, setRunDialogMode] = useState<'all' | 'selection'>('selection');
  const [selectionRunError, setSelectionRunError] = useState<string | null>(null);

  useEffect(() => {
    setSelectedTaskNames([]);
    setSelectedGroupNames([]);
    setSelectionMode('exact');
    setSelectedFlowName('');
    setSelectionStateFilter('all');
    setRunDialogOpen(false);
    setRunDialogMode('selection');
    setSelectionRunError(null);
  }, [selectedWorkflow]);

  useEffect(() => {
    if (!isMobileLayout) {
      setRunDialogOpen(false);
    }
  }, [isMobileLayout]);

  const nodeOrder = useMemo(() => activeGraphNodes.map((node) => node.id), [activeGraphNodes]);

  const depsByNode = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const node of activeGraphNodes) {
      map.set(node.id, [...(node.deps ?? [])]);
    }
    return map;
  }, [activeGraphNodes]);

  const reverseByNode = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const node of activeGraphNodes) {
      map.set(node.id, []);
    }
    for (const node of activeGraphNodes) {
      for (const dep of node.deps ?? []) {
        if (!map.has(dep)) map.set(dep, []);
        map.get(dep)!.push(node.id);
      }
    }
    return map;
  }, [activeGraphNodes]);

  useEffect(() => {
    const available = new Set(nodeOrder);
    setSelectedTaskNames((prev) => prev.filter((name) => available.has(name)));
  }, [nodeOrder]);

  const taskGroups = workflowDetail?.task_groups ?? {};
  const taskGroupEntries = useMemo(
    () => Object.entries(taskGroups).sort(([a], [b]) => a.localeCompare(b)),
    [taskGroups]
  );

  const expandedGroupTasks = useMemo(() => {
    const tasks = selectedGroupNames.flatMap((groupName) => taskGroups[groupName] ?? []);
    return unique(tasks);
  }, [selectedGroupNames, taskGroups]);

  const requestedTaskNames = useMemo(
    () => unique([...selectedTaskNames, ...expandedGroupTasks]),
    [selectedTaskNames, expandedGroupTasks]
  );

  const previewResolvedTaskNames = useMemo(
    () => resolveSelection(nodeOrder, depsByNode, reverseByNode, requestedTaskNames, selectionMode),
    [nodeOrder, depsByNode, reverseByNode, requestedTaskNames, selectionMode]
  );

  const exactModeMissingDependencyTasks = useMemo(() => {
    if (selectionMode !== 'exact') return [] as string[];
    const selectedSet = new Set(requestedTaskNames);
    const missing = new Set<string>();
    for (const taskName of requestedTaskNames) {
      for (const dep of depsByNode.get(taskName) ?? []) {
        if (!selectedSet.has(dep)) {
          missing.add(dep);
        }
      }
    }
    return [...missing].sort();
  }, [depsByNode, requestedTaskNames, selectionMode]);

  const materializationRows = useMemo<TaskMaterializationRow[]>(() => {
    if (workflowTaskStates.length > 0) {
      return workflowTaskStates.map((row) => ({
        task_name: row.task_name,
        last_status: row.last_status,
        finished_at: row.finished_at,
        success_at: row.success_at,
        failure_at: row.failure_at,
      }));
    }
    if (!workflowDetail?.tasks) {
      return [];
    }
    return workflowDetail.tasks.map((task) => ({
      task_name: task.name,
      last_status: task.last_state?.status,
      finished_at: task.last_state?.finished_at,
      success_at: task.last_state?.success_at,
      failure_at: task.last_state?.failure_at,
    }));
  }, [workflowTaskStates, workflowDetail]);

  const selectionStateOptions = useMemo(() => {
    const statuses = new Set<string>();
    materializationRows.forEach((row) => {
      if (row.last_status) {
        statuses.add(row.last_status);
      }
    });
    return ['all', 'never', ...[...statuses].sort()];
  }, [materializationRows]);

  const stateFilteredTaskNames = useMemo(() => {
    const available = new Set(nodeOrder);
    return materializationRows
      .filter((row) => {
        if (selectionStateFilter === 'all') return true;
        if (selectionStateFilter === 'never') return !row.last_status;
        return row.last_status === selectionStateFilter;
      })
      .map((row) => row.task_name)
      .filter((taskName) => available.has(taskName));
  }, [materializationRows, nodeOrder, selectionStateFilter]);

  const exactModeUnmaterializedDependencyCount = useMemo(() => {
    if (selectionMode !== 'exact' || exactModeMissingDependencyTasks.length === 0) {
      return 0;
    }
    const successfulTasks = new Set(
      materializationRows
        .filter((row) => (row.last_status ?? '').toLowerCase() === 'success')
        .map((row) => row.task_name)
    );
    return exactModeMissingDependencyTasks.filter((taskName) => !successfulTasks.has(taskName)).length;
  }, [exactModeMissingDependencyTasks, materializationRows, selectionMode]);

  const handleToggleTaskGroup = (groupName: string) => {
    setSelectedGroupNames((prev) => (
      prev.includes(groupName)
        ? prev.filter((name) => name !== groupName)
        : [...prev, groupName]
    ));
  };

  const handleExpandSelection = (mode: Exclude<TaskSelectionMode, 'exact'>) => {
    setSelectedTaskNames((prev) => resolveSelection(nodeOrder, depsByNode, reverseByNode, prev, mode));
  };

  const handleUseStateFilter = () => {
    setSelectedTaskNames(stateFilteredTaskNames);
    setSelectedGroupNames([]);
  };

  const handleAddStateFilter = () => {
    setSelectedTaskNames((prev) => unique([...prev, ...stateFilteredTaskNames]));
  };

  const handleUseResolved = () => {
    setSelectedTaskNames(previewResolvedTaskNames);
    setSelectedGroupNames([]);
  };

  return {
    nodeOrder,
    depsByNode,
    reverseByNode,
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
  };
}
