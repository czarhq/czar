import { useMemo } from 'react';
import type {
  GraphNode,
  RunDetail,
  TaskProgress,
  WorkflowInfo,
  WorkflowTaskStateListItem,
} from '../types';

interface UseGraphPresentationDataArgs {
  runDetail: RunDetail | null;
  workflowDetail: WorkflowInfo | null;
  workflowTaskStates: WorkflowTaskStateListItem[];
  workflowHistoryRuns: RunDetail[];
}

interface GraphPresentationData {
  runGraphNodes: GraphNode[];
  runStatusMap: Record<string, string>;
  runProgressMap: Record<string, TaskProgress | undefined>;
  workflowGraphNodes: GraphNode[];
  workflowStatusMap: Record<string, string>;
  workflowGridTaskNames: string[];
}

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

function sanitizeTaskNames(taskNames: string[]): string[] {
  const cleaned = [...new Set(taskNames.map((name) => name.trim()).filter(Boolean))];
  return cleaned.length > 1 ? cleaned.filter((name) => name.toLowerCase() !== 'task') : cleaned;
}

function buildRunGraphNodes(runDetail: RunDetail): GraphNode[] {
  const hasSelectionMetadata = Boolean(runDetail.run.task_selection);
  const resolved = uniqueCleanNames(runDetail.run.task_selection?.resolved_tasks ?? []);
  if (hasSelectionMetadata && resolved.length > 0) {
    const resolvedSet = new Set(resolved);
    const defs = runDetail.workflow?.tasks ?? [];
    const byName = new Map(defs.map((task) => [task.name.trim(), task]));
    const taskRowsByName = new Map(runDetail.tasks.map((task) => [task.name.trim(), task]));
    return resolved.map((taskName) => {
      const taskDef = byName.get(taskName);
      const taskRow = taskRowsByName.get(taskName);
      const deps = uniqueCleanNames(
        (taskDef?.depends_on ?? taskRow?.depends_on ?? []).filter((dep) => resolvedSet.has(dep.trim()))
      );
      return { id: taskName, deps };
    });
  }

  if (hasSelectionMetadata) {
    const byName = new Map<string, GraphNode>();
    for (const task of runDetail.tasks) {
      const id = task.name.trim();
      if (!id) continue;
      const deps = uniqueCleanNames(task.depends_on ?? []);
      const existing = byName.get(id);
      if (!existing) {
        byName.set(id, { id, deps });
      } else {
        byName.set(id, { id, deps: uniqueCleanNames([...existing.deps, ...deps]) });
      }
    }
    return [...byName.values()];
  }

  // For full-workflow runs without explicit selection, run tasks can be briefly
  // empty while scheduler/task rows catch up. Fall back to workflow definition.
  const workflowTasks = runDetail.workflow?.tasks ?? [];
  if (workflowTasks.length > 0) {
    return workflowTasks.map((task) => ({
      id: task.name.trim(),
      deps: uniqueCleanNames(task.depends_on ?? []),
    }))
      .filter((task) => task.id);
  }

  const byName = new Map<string, GraphNode>();
  for (const task of runDetail.tasks) {
    const id = task.name.trim();
    if (!id) continue;
    const deps = uniqueCleanNames(task.depends_on ?? []);
    const existing = byName.get(id);
    if (!existing) {
      byName.set(id, { id, deps });
    } else {
      byName.set(id, { id, deps: uniqueCleanNames([...existing.deps, ...deps]) });
    }
  }
  return [...byName.values()];
}

export function useGraphPresentationData({
  runDetail,
  workflowDetail,
  workflowTaskStates,
  workflowHistoryRuns,
}: UseGraphPresentationDataArgs): GraphPresentationData {
  // O(1) topology key: task topology is fixed at run start and never changes during polling.
  // Using run ID + workflow snapshot + task count + selection count covers all real topology changes.
  // O(1) topology key: task topology is fixed at run start and never changes during polling.
  // run.id + workflow name + workflow task count + run task count + selection count
  // covers all real topology changes without O(n) JSON.stringify on every render.
  const runGraphNodesKey = runDetail
    ? [
        runDetail.run.id,
        runDetail.workflow?.name ?? '',
        runDetail.workflow?.tasks?.length ?? 0,
        runDetail.tasks.length,
        (runDetail.run.task_selection?.resolved_tasks ?? []).length,
      ].join(':')
    : '';

  const runGraphNodes = useMemo<GraphNode[]>(() => {
    if (!runDetail) return [];
    return buildRunGraphNodes(runDetail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runGraphNodesKey]);

  // Avoiding JSON.stringify here: the per-node cache in useDecoratedNodes handles
  // unchanged-status nodes cheaply, and O(n) cache checks cost less than O(n) stringify.
  const runStatusMap = useMemo<Record<string, string>>(
    () => runDetail?.tasks
      ? Object.fromEntries(runDetail.tasks.map((t) => [t.name, t.status]))
      : {},
    [runDetail?.tasks]
  );

  const runProgressMap = useMemo<Record<string, TaskProgress | undefined>>(
    () => runDetail?.tasks
      ? Object.fromEntries(runDetail.tasks.map((t) => [t.name, t.progress]))
      : {},
    [runDetail?.tasks]
  );

  const workflowGraphNodes = useMemo<GraphNode[]>(() => {
    if (!workflowDetail?.tasks) return [];
    return workflowDetail.tasks.map((task) => ({ id: task.name, deps: task.depends_on || [] }));
  }, [workflowDetail]);

  const workflowStatusMap = useMemo<Record<string, string>>(() => {
    if (workflowTaskStates.length > 0) {
      return Object.fromEntries(
        workflowTaskStates
          .filter((taskState) => Boolean(taskState.last_status))
          .map((taskState) => [taskState.task_name, taskState.last_status!])
      );
    }
    if (!workflowDetail?.tasks) return {};
    return Object.fromEntries(
      workflowDetail.tasks
        .filter((task) => Boolean(task.last_state?.status))
        .map((task) => [task.name, task.last_state!.status])
    );
  }, [workflowTaskStates, workflowDetail]);

  const workflowGridTaskNames = useMemo<string[]>(() => {
    if (workflowDetail?.tasks?.length) {
      return sanitizeTaskNames(workflowDetail.tasks.map((task) => task.name));
    }
    const names = new Set<string>();
    workflowHistoryRuns.forEach((run) => {
      run.tasks.forEach((task) => names.add(task.name));
    });
    return sanitizeTaskNames([...names]).sort();
  }, [workflowDetail, workflowHistoryRuns]);

  return {
    runGraphNodes,
    runStatusMap,
    runProgressMap,
    workflowGraphNodes,
    workflowStatusMap,
    workflowGridTaskNames,
  };
}
