import { useEffect, useRef } from 'react';
import { fetchRunDetail } from '../api';
import type { RunDetail } from '../types';
import type { PollCadence } from './usePollCadence';
import { TERMINAL_STATUSES, isTerminalTransition } from './runStatus';

interface UseRunDetailPollingArgs {
  selectedRun: string;
  pollCadence: PollCadence;
  runDetail: RunDetail | null;
  setRunDetail: (detail: RunDetail | null) => void;
  onTerminalTransition: () => void;
}

const ACTIVE_TASK_STATUSES = new Set(['pending', 'dispatched', 'running', 'paused']);
const MAX_TERMINAL_GRAPH_CATCHUP_POLLS = 8;

function hasGraphData(detail: RunDetail): boolean {
  if (detail.run.task_selection) {
    if ((detail.run.task_selection.resolved_tasks?.length ?? 0) > 0) return true;
    return (detail.tasks?.length ?? 0) > 0;
  }
  if ((detail.tasks?.length ?? 0) > 0) return true;
  if ((detail.workflow?.tasks?.length ?? 0) > 0) return true;
  return false;
}

function hasSettledTerminalTaskStates(detail: RunDetail): boolean {
  if (!TERMINAL_STATUSES.has(detail.run.status)) return true;
  return !detail.tasks.some((task) => ACTIVE_TASK_STATUSES.has(task.status.toLowerCase()));
}

export function useRunDetailPolling({
  selectedRun,
  pollCadence,
  runDetail,
  setRunDetail,
  onTerminalTransition,
}: UseRunDetailPollingArgs): void {
  const runDetailRef = useRef(runDetail);
  runDetailRef.current = runDetail;

  useEffect(() => {
    if (!selectedRun) {
      setRunDetail(null);
      return;
    }

    let cancelled = false;
    let timeoutId: number | null = null;
    let inFlight = false;
    let failureCount = 0;
    let terminalGraphCatchupPolls = 0;
    let previousStatus = runDetailRef.current?.run.id === selectedRun
      ? runDetailRef.current.run.status
      : null;

    const schedule = (delayMs: number) => {
      if (cancelled) return;
      timeoutId = window.setTimeout(() => {
        void pollOnce();
      }, delayMs);
    };

    const pollOnce = async () => {
      if (cancelled || inFlight) return;
      inFlight = true;
      try {
        // Full fetch: the selected run shows task logs, output and errors.
        const detail = await fetchRunDetail(selectedRun);
        if (cancelled) return;
        // The workflow is omitted when the backend can't load it; keep the last one.
        const prev = runDetailRef.current;
        if (prev && prev.run.id === detail.run.id) {
          setRunDetail({ ...detail, workflow: detail.workflow ?? prev.workflow });
        } else {
          setRunDetail(detail);
        }

        const isTerminal = TERMINAL_STATUSES.has(detail.run.status);
        if (isTerminalTransition(previousStatus, detail.run.status)) {
          onTerminalTransition();
        }
        previousStatus = detail.run.status;
        failureCount = 0;

        // Keep polling terminal runs briefly until graph data is available.
        // This avoids a race where run status is terminal before task/detail
        // rows are fully visible in API responses.
        if (!isTerminal) {
          terminalGraphCatchupPolls = 0;
          schedule(pollCadence.successMs);
        } else if (!hasGraphData(detail)) {
          if (terminalGraphCatchupPolls < MAX_TERMINAL_GRAPH_CATCHUP_POLLS) {
            terminalGraphCatchupPolls += 1;
            schedule(pollCadence.successMs);
          } else {
            terminalGraphCatchupPolls = 0;
          }
        } else if (!hasSettledTerminalTaskStates(detail)) {
          // Keep polling while run is terminal but task rows still show active states.
          // This avoids stale node colors that otherwise require manual refresh.
          terminalGraphCatchupPolls = 0;
          schedule(Math.min(5_000, Math.max(1_000, pollCadence.successMs)));
        } else {
          terminalGraphCatchupPolls = 0;
          // Keep a low-frequency heartbeat for the selected run to avoid stale UI
          // if backend status propagation lags behind an earlier terminal snapshot.
          schedule(15_000);
        }
      } catch (error) {
        if (!cancelled) {
          console.error(error);
          failureCount += 1;
          const backoff = Math.min(
            pollCadence.maxBackoffMs,
            pollCadence.errorBaseMs * (2 ** Math.min(failureCount, 5))
          );
          schedule(backoff);
        }
      } finally {
        inFlight = false;
      }
    };

    void pollOnce();

    return () => {
      cancelled = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
    };
  }, [
    onTerminalTransition,
    pollCadence.errorBaseMs,
    pollCadence.maxBackoffMs,
    pollCadence.successMs,
    selectedRun,
    setRunDetail,
  ]);
}
