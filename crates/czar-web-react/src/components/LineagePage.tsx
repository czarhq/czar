import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { fetchWorkflowDetail, fetchWorkflows } from '../api';
import type { GraphNode, WorkflowInfo } from '../types';
import { useTheme } from '../hooks/useTheme';
import Graph from './Graph';
import { collapseGraphGroups } from './groupedGraphUtils';
import type { GraphOrientation } from './graphUtils';

const LINEAGE_PAGE_SIZE = 100;

interface WorkflowLoadFailure {
  name: string;
  error: string;
}

function workflowRootId(name: string): string {
  return `workflow:${name}`;
}

function workflowTaskId(workflowName: string, taskName: string): string {
  return `${workflowName}/${taskName}`;
}

export default function LineagePage({ isMobileLayout = false }: { isMobileLayout?: boolean }) {
  const { effectiveTheme } = useTheme();
  const [workflowNames, setWorkflowNames] = useState<string[]>([]);
  const [workflowDetails, setWorkflowDetails] = useState<Map<string, WorkflowInfo>>(new Map());
  const [failedWorkflows, setFailedWorkflows] = useState<WorkflowLoadFailure[]>([]);
  const [orientation, setOrientation] = useState<GraphOrientation>('LR');
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());
  const requestIdRef = useRef(0);

  useEffect(() => {
    const requestId = ++requestIdRef.current;

    async function loadLineage(): Promise<void> {
      setLoading(true);
      setLoadError(null);
      setFailedWorkflows([]);

      try {
        const firstPage = await fetchWorkflows(LINEAGE_PAGE_SIZE, 0, '');
        if (requestId !== requestIdRef.current) return;

        const names = firstPage.items.map((workflow) => workflow.name);
        const totalPages = Math.ceil(firstPage.total / LINEAGE_PAGE_SIZE);

        for (let page = 1; page < totalPages; page += 1) {
          const response = await fetchWorkflows(LINEAGE_PAGE_SIZE, page * LINEAGE_PAGE_SIZE, '');
          if (requestId !== requestIdRef.current) return;
          names.push(...response.items.map((workflow) => workflow.name));
        }

        const uniqueNames = [...new Set(names)];
        setWorkflowNames(uniqueNames);

        const detailResults = await Promise.all(
          uniqueNames.map(async (name) => {
            try {
              const detail = await fetchWorkflowDetail(name);
              return { name, detail, error: null as string | null };
            } catch (error) {
              return {
                name,
                detail: null,
                error: error instanceof Error ? error.message : 'Failed to load workflow detail.',
              };
            }
          }),
        );
        if (requestId !== requestIdRef.current) return;

        const nextDetails = new Map<string, WorkflowInfo>();
        const failures: WorkflowLoadFailure[] = [];
        for (const result of detailResults) {
          if (result.detail) {
            nextDetails.set(result.name, result.detail);
          } else if (result.error) {
            failures.push({ name: result.name, error: result.error });
          }
        }

        setWorkflowDetails(nextDetails);
        setFailedWorkflows(failures);
      } catch (error) {
        if (requestId !== requestIdRef.current) return;
        setWorkflowNames([]);
        setWorkflowDetails(new Map());
        setFailedWorkflows([]);
        setLoadError(error instanceof Error ? error.message : 'Failed to load workflow lineage.');
      } finally {
        if (requestId !== requestIdRef.current) return;
        setLoading(false);
      }
    }

    void loadLineage();
  }, []);

  const graphNodes = useMemo<GraphNode[]>(() => {
    const nodes: GraphNode[] = [];

    for (const workflowName of workflowNames) {
      const detail = workflowDetails.get(workflowName);
      if (!detail) continue;

      const rootId = workflowRootId(workflowName);
      nodes.push({
        id: rootId,
        deps: [],
        label: workflowName,
        exportLabel: workflowName,
      });

      for (const task of detail.tasks) {
        const taskDeps = task.depends_on.length > 0
          ? task.depends_on.map((dep) => workflowTaskId(workflowName, dep))
          : [rootId];

        nodes.push({
          id: workflowTaskId(workflowName, task.name),
          deps: taskDeps,
          label: task.name,
          exportLabel: `${workflowName}/${task.name}`,
        });
      }
    }

    return nodes;
  }, [workflowDetails, workflowNames]);

  // Build task groups for two collapse levels:
  // 1. Workflow-level: the whole workflow (root + all tasks) as one collapsible unit.
  // 2. Task-group-level: each WorkflowInfo.task_groups entry within a workflow.
  const taskGroups = useMemo<Record<string, string[]>>(() => {
    const groups: Record<string, string[]> = {};
    for (const workflowName of workflowNames) {
      const detail = workflowDetails.get(workflowName);
      if (!detail) continue;
      // Workflow-level group
      groups[workflowName] = [
        workflowRootId(workflowName),
        ...detail.tasks.map((t) => workflowTaskId(workflowName, t.name)),
      ];
      // Task-group-level groups within this workflow
      for (const [groupName, taskNames] of Object.entries(detail.task_groups)) {
        const lineageKey = `${workflowName}:${groupName}`;
        groups[lineageKey] = taskNames.map((t) => workflowTaskId(workflowName, t));
      }
    }
    return groups;
  }, [workflowDetails, workflowNames]);

  // Collect condition_expr and other params for conditional gate display.
  const nodeParams = useMemo<Record<string, Record<string, unknown>>>(() => {
    const params: Record<string, Record<string, unknown>> = {};
    for (const [workflowName, detail] of workflowDetails.entries()) {
      for (const task of detail.tasks) {
        if (task.params && Object.keys(task.params).length > 0) {
          params[workflowTaskId(workflowName, task.name)] = task.params;
        }
      }
    }
    return params;
  }, [workflowDetails]);

  const displayGraphNodes = useMemo(
    () => collapseGraphGroups(graphNodes, taskGroups, collapsedGroups),
    [graphNodes, taskGroups, collapsedGroups],
  );

  const handleToggleGroup = useCallback((groupName: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupName)) next.delete(groupName);
      else next.add(groupName);
      return next;
    });
  }, []);

  const groupNames = Object.keys(taskGroups).sort();
  const hasGroups = workflowNames.length > 0;
  // "Collapse All" only collapses at the workflow level (outermost group per workflow).
  // Task-group-level sub-groups overlap with workflow-level groups, so collapsing both
  // simultaneously causes tasks to have multiple memberships and be skipped entirely.
  const allCollapsed = workflowNames.length > 0 && workflowNames.every((g) => collapsedGroups.has(g));
  const allExpanded = hasGroups && groupNames.every((g) => !collapsedGroups.has(g));

  const handleCollapseAll = useCallback(() => {
    setCollapsedGroups(new Set(workflowNames));
  }, [workflowNames]);

  const handleExpandAll = useCallback(() => {
    setCollapsedGroups(new Set());
  }, []);

  const loadedWorkflowCount = workflowDetails.size;
  const taskCount = useMemo(
    () => [...workflowDetails.values()].reduce((sum, workflow) => sum + workflow.tasks.length, 0),
    [workflowDetails],
  );

  if (loading) {
    return (
      <div className="lineage-page">
        <p style={{ color: 'var(--muted)', padding: '2rem' }}>Loading combined workflow graph…</p>
      </div>
    );
  }

  if (loadError) {
    return (
      <div className="lineage-page">
        <p style={{ color: 'var(--danger)', padding: '2rem' }}>{loadError}</p>
      </div>
    );
  }

  if (workflowNames.length === 0 || graphNodes.length === 0) {
    return (
      <div className="lineage-page">
        <p style={{ color: 'var(--muted)', padding: '2rem' }}>No workflows found.</p>
      </div>
    );
  }

  return (
    <div className="lineage-page lineage-page--graph">
      <div className="lineage-toolbar">
        <span className="lineage-count">
          Combined graph for {loadedWorkflowCount} workflow{loadedWorkflowCount !== 1 ? 's' : ''} and {taskCount} task{taskCount !== 1 ? 's' : ''}
        </span>
        <div className="lineage-toolbar-actions">
          <label style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '13px', color: 'var(--muted)' }}>
            Orientation
            <select
              value={orientation}
              onChange={(e) => setOrientation(e.target.value as GraphOrientation)}
              className="graph-options-select"
              style={{ fontSize: '12px', padding: '2px 6px' }}
            >
              <option value="LR">Left-Right</option>
              <option value="TB">Top-Bottom</option>
            </select>
          </label>
        </div>
      </div>

      {failedWorkflows.length > 0 && (
        <div className="lineage-failures">
          <strong>Skipped {failedWorkflows.length} workflow{failedWorkflows.length !== 1 ? 's' : ''}:</strong>
          <ul className="lineage-failure-list">
            {failedWorkflows.map((failure) => (
              <li key={failure.name}>
                <code>{failure.name}</code>
                {' '}
                {failure.error}
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="lineage-graph-shell">
        <div className="panel lineage-graph-panel">
          {hasGroups && (
            <div className="graph-group-controls">
              <span className="graph-group-label">Groups</span>
              <button
                type="button"
                className="secondary"
                onClick={handleCollapseAll}
                disabled={allCollapsed}
              >
                Collapse All
              </button>
              <button
                type="button"
                className="secondary"
                onClick={handleExpandAll}
                disabled={allExpanded}
              >
                Expand All
              </button>
            </div>
          )}
          <Graph
            nodes={displayGraphNodes}
            nodeParams={nodeParams}
            isActive={true}
            orientation={orientation}
            onOrientationChange={setOrientation}
            downloadName="lineage-all-workflows"
            fitViewKey={`lineage-all:${loadedWorkflowCount}:${taskCount}:${failedWorkflows.length}`}
            colorMode={effectiveTheme}
            taskGroups={taskGroups}
            collapsedGroups={collapsedGroups}
            onToggleGroup={handleToggleGroup}
            isMobileLayout={isMobileLayout}
          />
        </div>
      </div>
    </div>
  );
}
