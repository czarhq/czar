import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent } from 'react';
import type { RunDetail, Task } from '../types';
import { fmtDateTime, statusClass } from '../utils';
import { useVirtualRows } from '../hooks/useVirtualRows';

interface RunGanttViewProps {
  runDetail: RunDetail;
  selectedTask: string | null;
  onTaskSelect: (taskName: string | null) => void;
}

interface TimelineSegment {
  kind: 'queued' | 'scheduled' | 'running';
  startMs: number;
  endMs: number;
  title: string;
}

interface TimelineRow {
  id: string;
  label: string;
  status: string;
  segments: TimelineSegment[];
}

interface TimelineEvent {
  label: string;
  ts: string;
  ms: number;
}

interface HoverCard {
  x: number;
  y: number;
  title: string;
  lines: string[];
}

const MIN_TIMELINE_SPAN_MS = 1_000;
const GANTT_VIRTUAL_THRESHOLD = 120;
const GANTT_ROW_HEIGHT = 59;
const TERMINAL_TASK_STATUSES = new Set(['success', 'failed', 'cancelled', 'canceled', 'skipped']);

function toMs(ts?: string): number | null {
  if (!ts) return null;
  const ms = Date.parse(ts);
  return Number.isFinite(ms) ? ms : null;
}

function pct(ms: number, startMs: number, endMs: number): number {
  if (endMs <= startMs) return 0;
  return ((ms - startMs) / (endMs - startMs)) * 100;
}

function clampMs(ms: number, startMs: number, endMs: number): number {
  return Math.min(endMs, Math.max(startMs, ms));
}

function fmtElapsed(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.round(durationMs / 1_000));
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}h ${minutes}m ${seconds}s`;
  if (minutes > 0) return `${minutes}m ${seconds}s`;
  return `${seconds}s`;
}

function fmtTimestamp(ms: number | null): string {
  if (ms === null) return '—';
  return fmtDateTime(new Date(ms).toISOString());
}

function durationMs(startMs: number | null, endMs: number | null): number | null {
  if (startMs === null || endMs === null) return null;
  if (endMs < startMs) return null;
  return endMs - startMs;
}

function fmtDurationOrDash(ms: number | null): string {
  if (ms === null) return '—';
  return fmtElapsed(ms);
}

function buildTaskSegments(runDetail: RunDetail, rangeEndMs: number): TimelineRow[] {
  const rows = runDetail.tasks.map((task) => {
    const dispatchedMs = toMs(task.dispatched_at);
    const startedMs = toMs(task.started_at);
    const finishedMs = toMs(task.finished_at);
    const statusLower = task.status.toLowerCase();
    const isTerminal = TERMINAL_TASK_STATUSES.has(statusLower);
    const segments: TimelineSegment[] = [];

    if (dispatchedMs && startedMs && startedMs > dispatchedMs) {
      segments.push({
        kind: 'scheduled',
        startMs: dispatchedMs,
        endMs: startedMs,
        title: `Scheduled/dispatch wait (${fmtDateTime(task.dispatched_at!)} -> ${fmtDateTime(task.started_at!)})`,
      });
    }

    if (startedMs) {
      const runningEndMs = finishedMs ?? (isTerminal ? startedMs : rangeEndMs);
      if (runningEndMs >= startedMs) {
        segments.push({
          kind: 'running',
          startMs: startedMs,
          endMs: runningEndMs,
          title: finishedMs
            ? `Running (${fmtDateTime(task.started_at!)} -> ${fmtDateTime(task.finished_at!)})`
            : `Running (${fmtDateTime(task.started_at!)} -> now)`,
        });
      }
    } else if (dispatchedMs && !isTerminal) {
      const waitEndMs = finishedMs ?? rangeEndMs;
      if (waitEndMs > dispatchedMs) {
        segments.push({
          kind: 'scheduled',
          startMs: dispatchedMs,
          endMs: waitEndMs,
          title: finishedMs
            ? `Scheduled/dispatch wait (${fmtDateTime(task.dispatched_at!)} -> ${fmtDateTime(task.finished_at!)})`
            : `Scheduled/dispatch wait (${fmtDateTime(task.dispatched_at!)} -> now)`,
        });
      }
    }

    return {
      id: task.name,
      label: task.name,
      status: task.status,
      segments,
    };
  });
  rows.sort((a, b) => {
    const aStart = a.segments.length > 0 ? Math.min(...a.segments.map((segment) => segment.startMs)) : Number.MAX_SAFE_INTEGER;
    const bStart = b.segments.length > 0 ? Math.min(...b.segments.map((segment) => segment.startMs)) : Number.MAX_SAFE_INTEGER;
    if (aStart !== bStart) return aStart - bStart;
    return a.label.localeCompare(b.label);
  });
  return rows;
}

export default function RunGanttView({ runDetail, selectedTask, onTaskSelect }: RunGanttViewProps) {
  const timeline = useMemo(() => {
    const nowMs = Date.now();
    const runCreatedMs = toMs(runDetail.run.created_at) ?? nowMs;
    const runStartedMs = toMs(runDetail.run.started_at);
    const runFinishedMs = toMs(runDetail.run.finished_at);
    const runEndMs = runFinishedMs ?? nowMs;
    const minMs = runCreatedMs;
    const maxMs = runEndMs - minMs < MIN_TIMELINE_SPAN_MS ? minMs + MIN_TIMELINE_SPAN_MS : runEndMs;
    const taskRows = buildTaskSegments(runDetail, maxMs);
    const earliestDispatchMs = runDetail.tasks
      .map((task) => toMs(task.dispatched_at))
      .filter((ms): ms is number => ms !== null)
      .sort((a, b) => a - b)[0];
    const scheduledMs = earliestDispatchMs ?? runStartedMs;
    const systemEvents: TimelineEvent[] = [
      { label: 'Queued', ts: new Date(runCreatedMs).toISOString(), ms: runCreatedMs },
      ...(scheduledMs
        ? [{ label: 'Scheduled', ts: new Date(scheduledMs).toISOString(), ms: scheduledMs }]
        : []),
      ...(runStartedMs
        ? [{ label: 'Started', ts: new Date(runStartedMs).toISOString(), ms: runStartedMs }]
        : []),
      ...(runFinishedMs
        ? [{ label: 'Finished', ts: new Date(runFinishedMs).toISOString(), ms: runFinishedMs }]
        : []),
    ];

    const spanSeconds = Math.max(1, Math.round((maxMs - minMs) / 1_000));
    const timelineWidthPx = Math.min(3600, Math.max(760, spanSeconds * 10));
    const runQueuedDuration = durationMs(runCreatedMs, scheduledMs ?? null);
    const runScheduledDuration = durationMs(scheduledMs ?? null, runStartedMs ?? null);
    const runExecutionDuration = durationMs(runStartedMs ?? null, runFinishedMs ?? maxMs);
    const runTotalDuration = durationMs(runCreatedMs, maxMs);
    const tickCount = Math.min(16, Math.max(4, Math.round(timelineWidthPx / 140)));
    const ticks = Array.from({ length: tickCount + 1 }, (_unused, index) => {
      const ratio = index / tickCount;
      const ms = minMs + (maxMs - minMs) * ratio;
      return { index, leftPct: ratio * 100, ms };
    });

    return {
      runCreatedMs,
      runFinishedMs,
      minMs,
      maxMs,
      taskRows,
      systemEvents,
      timelineWidthPx,
      runQueuedDuration,
      runScheduledDuration,
      runExecutionDuration,
      runTotalDuration,
      ticks,
    };
  }, [runDetail.run.created_at, runDetail.run.finished_at, runDetail.run.started_at, runDetail.tasks]);

  const {
    runCreatedMs,
    minMs,
    maxMs,
    taskRows,
    systemEvents,
    timelineWidthPx,
    runQueuedDuration,
    runScheduledDuration,
    runExecutionDuration,
    runTotalDuration,
    ticks,
  } = timeline;

  const [hover, setHover] = useState<HoverCard | null>(null);
  const hoverFrameRef = useRef<number | null>(null);
  const pendingHoverRef = useRef<{ x: number; y: number } | null>(null);
  const taskByName = useMemo(
    () => new Map(runDetail.tasks.map((task) => [task.name, task])),
    [runDetail.tasks]
  );
  const virtualRows = useVirtualRows<HTMLDivElement>({
    count: taskRows.length,
    rowHeight: GANTT_ROW_HEIGHT,
    overscan: 6,
    enabled: taskRows.length > GANTT_VIRTUAL_THRESHOLD,
  });
  const visibleTaskRows = taskRows.slice(virtualRows.start, virtualRows.end);

  useEffect(() => () => {
    if (hoverFrameRef.current !== null) {
      cancelAnimationFrame(hoverFrameRef.current);
      hoverFrameRef.current = null;
    }
  }, []);

  const showHover = useCallback((event: ReactMouseEvent<HTMLElement>, title: string, lines: string[]) => {
    setHover({
      x: event.clientX + 12,
      y: event.clientY + 12,
      title,
      lines,
    });
  }, []);

  const commitHoverPosition = useCallback(() => {
    hoverFrameRef.current = null;
    const next = pendingHoverRef.current;
    if (!next) return;
    setHover((prev) => (prev ? { ...prev, x: next.x, y: next.y } : prev));
  }, []);

  const moveHover = useCallback((event: ReactMouseEvent<HTMLElement>) => {
    pendingHoverRef.current = { x: event.clientX + 12, y: event.clientY + 12 };
    if (hoverFrameRef.current !== null) return;
    hoverFrameRef.current = requestAnimationFrame(commitHoverPosition);
  }, [commitHoverPosition]);

  const hideHover = useCallback(() => {
    pendingHoverRef.current = null;
    setHover(null);
  }, []);

  const taskHoverLines = useCallback((task: Task): string[] => {
    const dispatchedMs = toMs(task.dispatched_at);
    const startedMs = toMs(task.started_at);
    const finishedMs = toMs(task.finished_at);
    const terminal = TERMINAL_TASK_STATUSES.has(task.status.toLowerCase());
    const activeEndMs = finishedMs ?? (terminal ? startedMs : maxMs);
    const queuedDuration = durationMs(runCreatedMs, dispatchedMs);
    const scheduledDuration = durationMs(dispatchedMs, startedMs);
    const runDuration = durationMs(startedMs, activeEndMs ?? null);
    const trackedDuration = durationMs(dispatchedMs ?? startedMs ?? null, activeEndMs ?? null);

    return [
      `Status: ${task.status}`,
      `Queued at: ${fmtTimestamp(runCreatedMs)}`,
      `Dispatched at: ${fmtTimestamp(dispatchedMs)}`,
      `Started at: ${fmtTimestamp(startedMs)}`,
      `Finished at: ${fmtTimestamp(finishedMs)}`,
      `Queued duration: ${fmtDurationOrDash(queuedDuration)}`,
      `Scheduled duration: ${fmtDurationOrDash(scheduledDuration)}`,
      `Run duration: ${fmtDurationOrDash(runDuration)}`,
      `Tracked total: ${fmtDurationOrDash(trackedDuration)}`,
    ];
  }, [maxMs, runCreatedMs]);

  return (
    <div className="gantt-panel">
      <div className="gantt-system-events">
        {systemEvents.map((event) => (
          <span
            key={event.label}
            className="gantt-system-event"
            onMouseEnter={(eventMouse) => showHover(eventMouse, `Run ${event.label}`, [
              `Time: ${fmtDateTime(event.ts)}`,
              `Elapsed from run start: ${fmtElapsed(event.ms - minMs)}`,
              `Run queued: ${fmtDurationOrDash(runQueuedDuration)}`,
              `Run scheduled: ${fmtDurationOrDash(runScheduledDuration)}`,
              `Run execution: ${fmtDurationOrDash(runExecutionDuration)}`,
              `Run total: ${fmtDurationOrDash(runTotalDuration)}`,
            ])}
            onMouseMove={moveHover}
            onMouseLeave={hideHover}
          >
            <strong>{event.label}</strong>
          </span>
        ))}
      </div>

      <div className="gantt-scroll" ref={virtualRows.containerRef} onScroll={virtualRows.onScroll}>
        <div className="gantt-table" style={{ minWidth: `${timelineWidthPx + 220}px` }}>
          <div className="gantt-row gantt-row-scale" style={{ gridTemplateColumns: `minmax(180px, 220px) ${timelineWidthPx}px` }}>
            <div className="gantt-label">Timeline</div>
            <div className="gantt-track gantt-track-scale" aria-hidden="true">
              {ticks.map((tick) => (
                <span key={tick.index} className="gantt-tick" style={{ left: `${tick.leftPct}%` }}>
                  <span className="gantt-tick-label">{fmtElapsed(tick.ms - minMs)}</span>
                </span>
              ))}
            </div>
          </div>

          <div className="gantt-row gantt-row-system" style={{ gridTemplateColumns: `minmax(180px, 220px) ${timelineWidthPx}px` }}>
            <div className="gantt-label">Run Lifecycle</div>
            <div className="gantt-track" aria-label="System timeline events">
              {systemEvents.map((event, index) => {
                const left = pct(clampMs(event.ms, minMs, maxMs), minMs, maxMs);
                const previous = index > 0 ? systemEvents[index - 1] : null;
                const sincePrevious = previous ? durationMs(previous.ms, event.ms) : null;
                return (
                  <span
                    key={event.label}
                    className="gantt-system-marker"
                    style={{ left: `${left}%` }}
                    aria-label={`${event.label}: ${fmtDateTime(event.ts)}`}
                    onMouseEnter={(eventMouse) => showHover(eventMouse, `Run ${event.label}`, [
                      `Time: ${fmtDateTime(event.ts)}`,
                      `Elapsed from run start: ${fmtElapsed(event.ms - minMs)}`,
                      ...(previous ? [`Since ${previous.label}: ${fmtDurationOrDash(sincePrevious)}`] : []),
                    ])}
                    onMouseMove={moveHover}
                    onMouseLeave={hideHover}
                  />
                );
              })}
            </div>
          </div>

          {virtualRows.paddingTop > 0 && (
            <div className="gantt-virtual-spacer" style={{ height: `${virtualRows.paddingTop}px` }} aria-hidden="true" />
          )}

          {visibleTaskRows.map((row) => {
            const isSelected = row.id === selectedTask;
            return (
              <div
                key={row.id}
                className={`gantt-row${isSelected ? ' selected' : ''}`}
                style={{ gridTemplateColumns: `minmax(180px, 220px) ${timelineWidthPx}px` }}
                onClick={() => onTaskSelect(isSelected ? null : row.id)}
              >
                <div
                  className="gantt-label"
                  onMouseEnter={(eventMouse) => {
                    const task = taskByName.get(row.id);
                    if (!task) return;
                    showHover(eventMouse, `Task ${row.id}`, taskHoverLines(task));
                  }}
                  onMouseMove={moveHover}
                  onMouseLeave={hideHover}
                >
                  <span className="gantt-task-name">{row.label}</span>
                  <span className={`badge ${statusClass(row.status)}`}>{row.status}</span>
                </div>
                <div className="gantt-track">
                  {row.segments.length === 0 && (
                    <span className="gantt-no-segment">
                      —
                    </span>
                  )}
                  {row.segments.map((segment, index) => {
                    const left = pct(clampMs(segment.startMs, minMs, maxMs), minMs, maxMs);
                    const right = pct(clampMs(segment.endMs, minMs, maxMs), minMs, maxMs);
                    const width = right - left;
                    if (width <= 0) return null;
                    return (
                      <span
                        key={`${row.id}-${segment.kind}-${index}`}
                        className={`gantt-segment ${segment.kind}`}
                        style={{ left: `${left}%`, width: `${width}%` }}
                        onMouseEnter={(eventMouse) => showHover(eventMouse, `${row.id} • ${segment.kind}`, [
                          `${segment.title}`,
                          `Start: ${fmtTimestamp(segment.startMs)}`,
                          `End: ${fmtTimestamp(segment.endMs)}`,
                          `Duration: ${fmtElapsed(segment.endMs - segment.startMs)}`,
                          `Elapsed start: ${fmtElapsed(segment.startMs - minMs)}`,
                          `Elapsed end: ${fmtElapsed(segment.endMs - minMs)}`,
                        ])}
                        onMouseMove={moveHover}
                        onMouseLeave={hideHover}
                      />
                    );
                  })}
                </div>
              </div>
            );
          })}

          {virtualRows.paddingBottom > 0 && (
            <div className="gantt-virtual-spacer" style={{ height: `${virtualRows.paddingBottom}px` }} aria-hidden="true" />
          )}
        </div>
      </div>

      {hover && (
        <div className="gantt-tooltip" style={{ left: `${hover.x}px`, top: `${hover.y}px` }}>
          <div className="gantt-tooltip-title">{hover.title}</div>
          {hover.lines.map((line, index) => (
            <div key={`${index}:${line}`} className="gantt-tooltip-line">{line}</div>
          ))}
        </div>
      )}
    </div>
  );
}
