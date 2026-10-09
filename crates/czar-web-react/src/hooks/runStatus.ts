export const TERMINAL_STATUSES = new Set(['success', 'failed', 'cancelled', 'canceled', 'skipped']);

// Only a change from a known non-terminal status counts; opening a finished run does not.
export function isTerminalTransition(previousStatus: string | null, status: string): boolean {
  return previousStatus !== null
    && !TERMINAL_STATUSES.has(previousStatus)
    && TERMINAL_STATUSES.has(status);
}
