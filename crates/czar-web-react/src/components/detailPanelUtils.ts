import type { RunDetail } from '../types';

export function normalizeRunTaskNames(run: RunDetail): string[] {
  const names = run.run.task_selection?.resolved_tasks?.length
    ? run.run.task_selection.resolved_tasks
    : run.tasks.map((task) => task.name);
  const seen = new Set<string>();
  return names.filter((name) => {
    if (!name || seen.has(name)) return false;
    seen.add(name);
    return true;
  });
}

export function taskSetSignature(taskNames: string[]): string {
  return [...new Set(taskNames)].sort().join('\u0000');
}
