import type { TaskSelectionMode } from '../types';

export function unique(values: string[]): string[] {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))];
}

function collectUpstream(depsByNode: Map<string, string[]>, seeds: string[]): Set<string> {
  const visited = new Set<string>();
  const stack = [...seeds];
  while (stack.length > 0) {
    const name = stack.pop()!;
    if (visited.has(name)) continue;
    visited.add(name);
    for (const dep of depsByNode.get(name) ?? []) {
      if (!visited.has(dep)) stack.push(dep);
    }
  }
  return visited;
}

function collectDownstream(reverseByNode: Map<string, string[]>, seeds: string[]): Set<string> {
  const visited = new Set<string>();
  const stack = [...seeds];
  while (stack.length > 0) {
    const name = stack.pop()!;
    if (visited.has(name)) continue;
    visited.add(name);
    for (const child of reverseByNode.get(name) ?? []) {
      if (!visited.has(child)) stack.push(child);
    }
  }
  return visited;
}

export function resolveSelection(
  nodeOrder: string[],
  depsByNode: Map<string, string[]>,
  reverseByNode: Map<string, string[]>,
  seeds: string[],
  mode: TaskSelectionMode
): string[] {
  const cleanSeeds = unique(seeds).filter((seed) => depsByNode.has(seed));
  if (cleanSeeds.length === 0) return [];
  let selected: Set<string>;
  if (mode === 'exact') {
    selected = new Set(cleanSeeds);
  } else if (mode === 'upstream') {
    selected = collectUpstream(depsByNode, cleanSeeds);
  } else if (mode === 'downstream') {
    selected = collectDownstream(reverseByNode, cleanSeeds);
  } else {
    const full = collectUpstream(depsByNode, cleanSeeds);
    const down = collectDownstream(reverseByNode, [...full]);
    selected = new Set([...full, ...down]);
  }
  return nodeOrder.filter((name) => selected.has(name));
}
