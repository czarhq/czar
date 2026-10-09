import type { GraphNode } from '../types';

export const GROUP_NODE_PREFIX = '__group__';

export function makeGroupNodeId(groupName: string): string {
  return `${GROUP_NODE_PREFIX}${groupName}`;
}

export function isGroupNode(nodeId: string): boolean {
  return nodeId.startsWith(GROUP_NODE_PREFIX);
}

export function getGroupNameFromNodeId(nodeId: string): string {
  return nodeId.slice(GROUP_NODE_PREFIX.length);
}

interface CollapsedGroupState {
  taskToGroup: Map<string, string>;
  groupTaskCounts: Map<string, number>;
}

function buildCollapsedGroupState(
  nodes: GraphNode[],
  taskGroups: Record<string, string[]>,
  collapsedGroups: Set<string>,
): CollapsedGroupState {
  const presentTaskIds = new Set(nodes.map((node) => node.id));
  const taskMemberships = new Map<string, string[]>();

  for (const [groupName, tasks] of Object.entries(taskGroups)) {
    if (!collapsedGroups.has(groupName)) continue;
    for (const task of tasks) {
      if (!presentTaskIds.has(task)) continue;
      const memberships = taskMemberships.get(task) ?? [];
      memberships.push(groupName);
      taskMemberships.set(task, memberships);
    }
  }

  const taskToGroup = new Map<string, string>();
  const groupTaskCounts = new Map<string, number>();

  for (const [task, memberships] of taskMemberships.entries()) {
    if (memberships.length !== 1) continue;
    const groupName = memberships[0];
    taskToGroup.set(task, groupName);
    groupTaskCounts.set(groupName, (groupTaskCounts.get(groupName) ?? 0) + 1);
  }

  return {
    taskToGroup,
    groupTaskCounts,
  };
}

/**
 * Collapse groups of tasks into single group nodes in the graph.
 * For each collapsed group, all tasks in the group become one node.
 * Dependencies crossing group boundaries are re-wired to/from the group node.
 */
export function collapseGraphGroups(
  nodes: GraphNode[],
  taskGroups: Record<string, string[]>,
  collapsedGroups: Set<string>,
): GraphNode[] {
  if (collapsedGroups.size === 0) return nodes;

  const { taskToGroup, groupTaskCounts } = buildCollapsedGroupState(nodes, taskGroups, collapsedGroups);
  if (taskToGroup.size === 0) return nodes;

  // Map a node id to its representative (group node id if collapsed, else itself)
  const resolveId = (id: string): string => {
    const groupName = taskToGroup.get(id);
    return groupName ? makeGroupNodeId(groupName) : id;
  };

  // Track group node deps and task counts
  const groupDepsMap = new Map<string, Set<string>>();
  for (const groupName of groupTaskCounts.keys()) {
    groupDepsMap.set(makeGroupNodeId(groupName), new Set<string>());
  }

  const resultNodes: GraphNode[] = [];

  for (const node of nodes) {
    const groupName = taskToGroup.get(node.id);
    if (groupName) {
      // Accumulate external deps for this group node
      const groupNodeId = makeGroupNodeId(groupName);
      const groupDeps = groupDepsMap.get(groupNodeId);
      if (groupDeps) {
        for (const dep of node.deps) {
          const resolvedDep = resolveId(dep);
          if (resolvedDep !== groupNodeId) {
            groupDeps.add(resolvedDep);
          }
        }
      }
    } else {
      // Remap deps that point into a collapsed group
      const remappedDeps = [...new Set(node.deps.map(resolveId))].filter(
        (dep) => dep !== node.id,
      );
      resultNodes.push({ ...node, deps: remappedDeps });
    }
  }

  // Add one node per collapsed group
  for (const [groupNodeId, deps] of groupDepsMap.entries()) {
    const groupName = getGroupNameFromNodeId(groupNodeId);
    const count = groupTaskCounts.get(groupName) ?? 0;
    resultNodes.push({
      id: groupNodeId,
      deps: [...deps],
      label: `▶ ${groupName} (${count})`,
      exportLabel: `${groupName} (${count})`,
    });
  }

  return resultNodes;
}

const STATUS_PRIORITY: Record<string, number> = {
  failed: 6,
  cancelled: 5,
  canceled: 5,
  skipped: 5,
  running: 4,
  dispatched: 4,
  paused: 3,
  success: 2,
  pending: 1,
};

/**
 * Build a status map that includes a derived status for each collapsed group node.
 * The group's status is the highest-priority status among its member tasks.
 */
export function buildGroupStatusEntries(
  nodes: GraphNode[],
  statusMap: Record<string, string>,
  taskGroups: Record<string, string[]>,
  collapsedGroups: Set<string>,
): Record<string, string> {
  if (collapsedGroups.size === 0) return {};

  const { taskToGroup, groupTaskCounts } = buildCollapsedGroupState(nodes, taskGroups, collapsedGroups);
  if (taskToGroup.size === 0) return {};

  const extra: Record<string, string> = {};
  for (const groupName of groupTaskCounts.keys()) {
    let bestStatus = '';
    let bestPriority = -1;
    for (const [taskName, memberGroupName] of taskToGroup.entries()) {
      if (memberGroupName !== groupName) continue;
      const status = (statusMap[taskName] ?? '').toLowerCase();
      const priority = STATUS_PRIORITY[status] ?? 0;
      if (priority > bestPriority) {
        bestPriority = priority;
        bestStatus = status;
      }
    }
    extra[makeGroupNodeId(groupName)] = bestStatus;
  }
  return extra;
}
