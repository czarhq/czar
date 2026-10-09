import type { RunDetail } from '../types';
import { fmtDateTime, fmtRunOccurredAtLabel, statusClass, truncate } from '../utils';
import { StatusIcon } from './StatusIcon';
import { useMemo, type CSSProperties } from 'react';
import { useVirtualRows } from '../hooks/useVirtualRows';

interface WorkflowRunsGridProps {
  workflowName: string;
  taskNames: string[];
  runs: RunDetail[];
  loading: boolean;
  nextScheduledAt?: string;
  selectedRunId?: string;
  selectedTaskName?: string;
  onCellClick: (runId: string, taskName: string | null) => void;
}

const GRID_TASK_VIRTUAL_THRESHOLD = 120;
const GRID_TASK_ROW_HEIGHT = 28;
const DUR_BAR_MIN_PX = 10;
const DUR_BAR_MAX_PX = 72;
const GRID_SKELETON_ROWS = 8;
const GRID_SKELETON_COLS = 10;

function fmtMs(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

function fmtNextScheduleLabel(nextScheduledAt?: string): string | null {
  return nextScheduledAt ? `Next: ${fmtDateTime(nextScheduledAt)}` : null;
}

function buildCellTitle(primary: string, nextScheduleLabel: string | null): string {
  return nextScheduleLabel ? `${primary} • ${nextScheduleLabel}` : primary;
}

export default function WorkflowRunsGrid({
  workflowName,
  taskNames,
  runs,
  loading,
  nextScheduledAt,
  selectedRunId,
  selectedTaskName,
  onCellClick,
}: WorkflowRunsGridProps) {
  const nextScheduleLabel = useMemo(
    () => fmtNextScheduleLabel(nextScheduledAt),
    [nextScheduledAt],
  );
  const longestTaskNameLen = useMemo(
    () => taskNames.reduce((max, name) => Math.max(max, name.length), 0),
    [taskNames]
  );
  const taskColWidth = useMemo(
    () => Math.min(380, Math.max(180, Math.round(longestTaskNameLen * 7.2 + 34))),
    [longestTaskNameLen]
  );
  const runDurationsMs = useMemo(() => {
    return new Map(runs.map((run) => {
      const startMs = run.run.started_at ? Date.parse(run.run.started_at) : null;
      const endMs = run.run.finished_at ? Date.parse(run.run.finished_at) : null;
      const ms = startMs !== null && endMs !== null && endMs >= startMs ? endMs - startMs : null;
      return [run.run.id, ms] as [string, number | null];
    }));
  }, [runs]);
  const maxDurationMs = useMemo(() => {
    let max = 0;
    for (const [, ms] of runDurationsMs) {
      if (ms !== null && ms > max) max = ms;
    }
    return max;
  }, [runDurationsMs]);

  const statusByRunTask = useMemo(() => {
    const byRun = new Map<string, Map<string, string>>();
    for (const run of runs) {
      const byTask = new Map<string, string>();
      for (const task of run.tasks) {
        byTask.set(task.name, task.status);
      }
      byRun.set(run.run.id, byTask);
    }
    return byRun;
  }, [runs]);
  const virtualRows = useVirtualRows<HTMLDivElement>({
    count: taskNames.length,
    rowHeight: GRID_TASK_ROW_HEIGHT,
    enabled: taskNames.length > GRID_TASK_VIRTUAL_THRESHOLD,
  });
  const visibleTaskNames = taskNames.slice(virtualRows.start, virtualRows.end);
  const gridStyle = { '--workflow-grid-task-col-width': `${taskColWidth}px` } as CSSProperties;
  const hasGridContent = runs.length > 0 || taskNames.length > 0;
  const showSkeleton = Boolean(workflowName) && loading && !hasGridContent;

  return (
    <div>
      <div className="panel-header">
        <h2>Grid</h2>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0 }}>
          {nextScheduleLabel && (
            <span
              className="text-sm"
              title={nextScheduleLabel}
              style={{ color: 'var(--muted)', whiteSpace: 'nowrap' }}
            >
              {nextScheduleLabel}
            </span>
          )}
          <span className="pill">{loading && !hasGridContent ? '…' : runs.length}</span>
        </div>
      </div>

      <div className="workflow-grid-body">
        {!workflowName ? (
          <p className="workflow-grid-placeholder">Select a workflow to view run grid.</p>
        ) : showSkeleton ? (
          <div className="workflow-grid-scroll workflow-grid-scroll--skeleton" style={gridStyle} aria-hidden="true">
            <table className="workflow-grid-table workflow-grid-table--skeleton">
              <tbody>
                <tr className="workflow-grid-duration-row">
                  <td className="workflow-grid-task-col workflow-grid-duration-label">
                    <span className="ui-skeleton-line ui-skeleton-line--label" />
                  </td>
                  {Array.from({ length: GRID_SKELETON_COLS }, (_, index) => (
                    <td key={`dur:${index}`} className="workflow-grid-cell">
                      <div className="workflow-grid-dur-cell">
                        <div
                          className="workflow-grid-dur-bar workflow-grid-dur-bar--skeleton"
                          style={{ height: `${20 + ((index * 7) % 42)}px` }}
                        />
                      </div>
                    </td>
                  ))}
                </tr>
                {Array.from({ length: GRID_SKELETON_ROWS }, (_, rowIndex) => (
                  <tr key={`row:${rowIndex}`}>
                    <td className="workflow-grid-task-col">
                      <span className="ui-skeleton-line ui-skeleton-line--task" />
                    </td>
                    {Array.from({ length: GRID_SKELETON_COLS }, (_, colIndex) => (
                      <td key={`cell:${rowIndex}:${colIndex}`} className="workflow-grid-cell">
                        <span className="workflow-grid-dot workflow-grid-dot--skeleton" />
                      </td>
                    ))}
                  </tr>
                ))}
                <tr className="workflow-grid-overall-row">
                  <td className="workflow-grid-task-col workflow-grid-overall-label">
                    <span className="ui-skeleton-line ui-skeleton-line--label" />
                  </td>
                  {Array.from({ length: GRID_SKELETON_COLS }, (_, index) => (
                    <td key={`overall:${index}`} className="workflow-grid-cell">
                      <span className="workflow-grid-dot workflow-grid-dot--skeleton workflow-grid-dot--skeleton-lg" />
                    </td>
                  ))}
                </tr>
              </tbody>
            </table>
          </div>
        ) : runs.length === 0 ? (
          <p className="workflow-grid-placeholder">No runs found for this workflow.</p>
        ) : taskNames.length === 0 ? (
          <p className="workflow-grid-placeholder">No workflow tasks available.</p>
        ) : (
          <div
            className="workflow-grid-scroll"
            style={gridStyle}
            ref={virtualRows.containerRef}
            onScroll={virtualRows.onScroll}
          >
            <table className="workflow-grid-table">
              <colgroup>
                <col style={{ width: `${taskColWidth}px` }} />
                {runs.map((run) => (
                  <col key={`col:${run.run.id}`} style={{ width: '13px' }} />
                ))}
              </colgroup>
              <tbody>
                <tr className="workflow-grid-duration-row">
                  <td className="workflow-grid-task-col workflow-grid-duration-label">Duration</td>
                  {runs.map((run) => {
                    const ms = runDurationsMs.get(run.run.id) ?? null;
                    const ratio = ms !== null && maxDurationMs > 0 ? ms / maxDurationMs : 0;
                    const barH = ms !== null
                      ? Math.max(DUR_BAR_MIN_PX, Math.round(ratio * DUR_BAR_MAX_PX))
                      : 0;
                    const isSelected = run.run.id === selectedRunId;
                    return (
                      <td
                        key={`dur:${run.run.id}`}
                        className={`workflow-grid-cell${isSelected ? ' workflow-grid-cell--selected' : ''}`}
                        onClick={() => onCellClick(run.run.id, null)}
                        title={buildCellTitle(
                          `${run.run.id} • ${fmtRunOccurredAtLabel(run.run)} • ${ms !== null ? fmtMs(ms) : 'no duration'}`,
                          nextScheduleLabel,
                        )}
                      >
                        <div className="workflow-grid-dur-cell">
                          {barH > 0 && (
                            <div
                              className={`workflow-grid-dur-bar ${statusClass(run.run.status)}`}
                              style={{ height: `${barH}px` }}
                            />
                          )}
                        </div>
                      </td>
                    );
                  })}
                </tr>
                {virtualRows.paddingTop > 0 && (
                  <tr className="virtual-spacer-row" aria-hidden="true">
                    <td colSpan={runs.length + 1} style={{ height: `${virtualRows.paddingTop}px` }} />
                  </tr>
                )}
                {visibleTaskNames.map((taskName) => {
                  const isSelectedTask = taskName === selectedTaskName;
                  return (
                    <tr key={taskName} className={isSelectedTask ? 'workflow-grid-row--selected' : undefined}>
                      <td
                        className="workflow-grid-task-col"
                        title={taskName}
                        style={{ cursor: 'pointer' }}
                        onClick={() => selectedRunId && onCellClick(selectedRunId, taskName)}
                      >
                        <span className="workflow-grid-task-text">{truncate(taskName, 30)}</span>
                      </td>
                      {runs.map((run) => {
                        const status = statusByRunTask.get(run.run.id)?.get(taskName) ?? null;
                        const cls = status ? statusClass(status) : 'none';
                        const isSelected = run.run.id === selectedRunId;
                        return (
                          <td
                            key={`${taskName}:${run.run.id}`}
                            className={`workflow-grid-cell${isSelected ? ' workflow-grid-cell--selected' : ''}`}
                            onClick={() => onCellClick(run.run.id, taskName)}
                            title={buildCellTitle(
                              `${run.run.id} • ${fmtRunOccurredAtLabel(run.run)} • ${taskName}: ${status ?? 'not started'}`,
                              nextScheduleLabel,
                            )}
                          >
                            {status
                              ? <StatusIcon status={status} size={11} className={`workflow-grid-icon workflow-grid-icon--${cls}`} />
                              : <span className="workflow-grid-dot none" />
                            }
                          </td>
                        );
                      })}
                    </tr>
                  );
                })}
                {virtualRows.paddingBottom > 0 && (
                  <tr className="virtual-spacer-row" aria-hidden="true">
                    <td colSpan={runs.length + 1} style={{ height: `${virtualRows.paddingBottom}px` }} />
                  </tr>
                )}
                <tr className="workflow-grid-overall-row">
                  <td className="workflow-grid-task-col workflow-grid-overall-label">Overall</td>
                  {runs.map((run) => {
                    const runStatus = run.run.status;
                    const isSelected = run.run.id === selectedRunId;
                    return (
                      <td
                        key={`overall:${run.run.id}`}
                        className={`workflow-grid-cell${isSelected ? ' workflow-grid-cell--selected' : ''}`}
                        onClick={() => onCellClick(run.run.id, null)}
                        title={buildCellTitle(
                          `${run.run.id} • ${fmtRunOccurredAtLabel(run.run)} • ${runStatus}`,
                          nextScheduleLabel,
                        )}
                      >
                        <StatusIcon status={runStatus} size={13} className={`workflow-grid-icon workflow-grid-icon--${statusClass(runStatus)}`} />
                      </td>
                    );
                  })}
                </tr>
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
