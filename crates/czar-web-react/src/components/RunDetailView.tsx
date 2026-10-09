import { useState, type MouseEvent as ReactMouseEvent } from 'react';
import type { RunDetail, TaskProgress, WorkflowTaskInfo } from '../types';
import { fmtDateTime, fmtDuration, truncate } from '../utils';
import { StatusBadge } from './StatusIcon';
import RunGanttView from './RunGanttView';
import { useVirtualRows } from '../hooks/useVirtualRows';

// HH:MM:SS.mmm — 12 chars followed by a space
const TS_RE = /^(\d{2}:\d{2}:\d{2}\.\d{3}) /;
const TASK_TABLE_VIRTUAL_THRESHOLD = 120;
const TASK_TABLE_ROW_HEIGHT = 41;
const LOGS_SELECTION_THRESHOLD = 40;
const TERMINAL_TASK_STATUSES = new Set(['success', 'failed', 'cancelled', 'canceled', 'skipped']);
const RUN_DETAIL_COLUMN_WIDTHS = ['220px', '96px', '120px', '180px', '180px', '96px', '280px'] as const;

interface LogLine {
  stream: 'O' | 'E' | 'S' | 'raw';
  ts: string | null;
  text: string;
}

function normalizeProgress(raw: TaskProgress | undefined): { current: number; total: number; percent: number } | null {
  if (!raw || typeof raw !== 'object') return null;
  const current = Number(raw.current);
  const total = Number(raw.total);
  if (!Number.isFinite(current) || !Number.isFinite(total) || total <= 0) return null;
  const ratio = Math.min(1, Math.max(0, current / total));
  return {
    current: Math.max(0, Math.round(current)),
    total: Math.max(1, Math.round(total)),
    percent: Math.round(ratio * 100),
  };
}

function parseLogLines(raw: string): LogLine[] {
  return raw.split('\n').filter((l) => l.length > 0).map((l) => {
    const stream = l.startsWith('O ') ? 'O' : l.startsWith('E ') ? 'E' : l.startsWith('S ') ? 'S' : null;
    if (stream) {
      const rest = l.slice(2);
      const m = TS_RE.exec(rest);
      return m ? { stream, ts: m[1], text: rest.slice(m[0].length) } : { stream, ts: null, text: rest };
    }
    return { stream: 'raw', ts: null, text: l };
  });
}

function RunDetailTableColGroup() {
  return (
    <colgroup>
      {RUN_DETAIL_COLUMN_WIDTHS.map((width, index) => (
        <col key={index} style={{ width }} />
      ))}
    </colgroup>
  );
}

function LogBlock({ taskName, logs, showDetails }: { taskName: string; logs: string | undefined; showDetails: boolean }) {
  const [copied, setCopied] = useState(false);

  const copyLogs = () => {
    if (!logs) return;
    const plain = parseLogLines(logs).map((l) => l.text).join('\n');
    navigator.clipboard.writeText(plain).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };

  const btnStyle: React.CSSProperties = { fontSize: '11px', padding: '1px 7px', background: 'transparent', border: '1px solid var(--border)', borderRadius: '3px', color: 'var(--muted)', cursor: 'pointer' };

  return (
    <div className="log-block">
      <div className="log-title" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
        <span>{taskName} logs</span>
        {logs && (
          <button type="button" onClick={copyLogs} style={btnStyle}>
            {copied ? 'Copied!' : 'Copy'}
          </button>
        )}
      </div>
      {logs ? (
        <pre style={{ userSelect: 'text', margin: 0 }}>
          {parseLogLines(logs).map((line, i) => {
            const color = line.stream === 'E' ? '#f97316' : line.stream === 'S' ? '#60a5fa' : undefined;
            const label = line.stream === 'O' ? 'out' : line.stream === 'E' ? 'err' : line.stream === 'S' ? 'sys' : '   ';
            return (
              <span key={i} style={{ color, display: 'block' }}>
                {showDetails && (
                  <span style={{ opacity: 0.45, marginRight: '6px', userSelect: 'none' }}>
                    [{label}]{line.ts ? ` ${line.ts}` : ''}
                  </span>
                )}
                {line.text}
              </span>
            );
          })}
        </pre>
      ) : (
        <p style={{ color: 'var(--muted)' }}>No logs yet.</p>
      )}
    </div>
  );
}

interface RunDetailViewProps {
  runDetail: RunDetail | null;
  loading?: boolean;
  activeTab: 'details' | 'gantt';
  selectedTask: string | null;
  selectedTaskDef: WorkflowTaskInfo | null;
  onTaskSelect: (taskName: string | null) => void;
  onTabLinkClick: (event: ReactMouseEvent<HTMLAnchorElement>, tab: 'details' | 'gantt') => void;
  tabHrefs: Record<'details' | 'gantt', string>;
  logDetails: boolean;
}

export default function RunDetailView({
  runDetail,
  loading = false,
  activeTab,
  selectedTask,
  selectedTaskDef,
  onTaskSelect,
  onTabLinkClick,
  tabHrefs,
  logDetails,
}: RunDetailViewProps) {
  const tasks = runDetail?.tasks ?? [];
  // For large runs, render logs only for a focused task to avoid heavy DOM cost.
  const focusedTask = selectedTask ? tasks.find((t) => t.name === selectedTask) : null;
  const logsRequireTaskSelection = tasks.length > LOGS_SELECTION_THRESHOLD && !focusedTask;
  const logTasks = logsRequireTaskSelection ? [] : (focusedTask ? [focusedTask] : tasks);
  const virtualTasks = useVirtualRows<HTMLDivElement>({
    count: tasks.length,
    rowHeight: TASK_TABLE_ROW_HEIGHT,
    enabled: tasks.length > TASK_TABLE_VIRTUAL_THRESHOLD,
  });
  const visibleTasks = tasks.slice(virtualTasks.start, virtualTasks.end);

  return (
    <>
      <div className="run-detail-tabs">
        <a
          href={tabHrefs.details}
          className={activeTab === 'details' ? 'tab active' : 'tab'}
          onClick={(event) => onTabLinkClick(event, 'details')}
        >
          Details
        </a>
        <a
          href={tabHrefs.gantt}
          className={activeTab === 'gantt' ? 'tab active' : 'tab'}
          onClick={(event) => onTabLinkClick(event, 'gantt')}
        >
          Gantt
        </a>
      </div>

      {loading ? (
        <div className="run-detail-skeleton" aria-hidden="true">
          <div className="run-detail-task-table-scroll">
            <table className="run-detail-table">
              <RunDetailTableColGroup />
              <thead>
                <tr>
                  <th>Task</th>
                  <th>Status</th>
                  <th>Progress</th>
                  <th>Started</th>
                  <th>Finished</th>
                  <th>Duration</th>
                  <th>Output</th>
                </tr>
              </thead>
              <tbody>
                {Array.from({ length: 8 }, (_, index) => (
                  <tr key={`skeleton:${index}`}>
                    <td><span className="ui-skeleton-line ui-skeleton-line--task" /></td>
                    <td><span className="ui-skeleton-pill" /></td>
                    <td><span className="ui-skeleton-line ui-skeleton-line--progress" /></td>
                    <td><span className="ui-skeleton-line ui-skeleton-line--meta" /></td>
                    <td><span className="ui-skeleton-line ui-skeleton-line--meta" /></td>
                    <td><span className="ui-skeleton-line ui-skeleton-line--meta" /></td>
                    <td><span className="ui-skeleton-line ui-skeleton-line--output" /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <div className="run-detail-log-list">
            {Array.from({ length: 2 }, (_, index) => (
              <div key={`log:${index}`} className="log-block">
                <div className="log-title">
                  <span className="ui-skeleton-line ui-skeleton-line--label" />
                </div>
                <div className="run-detail-skeleton-log">
                  <span className="ui-skeleton-line" />
                  <span className="ui-skeleton-line ui-skeleton-line--wide" />
                  <span className="ui-skeleton-line ui-skeleton-line--mid" />
                  <span className="ui-skeleton-line ui-skeleton-line--task" />
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : activeTab === 'gantt' ? (
        <RunGanttView runDetail={runDetail!} selectedTask={selectedTask} onTaskSelect={onTaskSelect} />
      ) : (
        <>
          <div
            className="run-detail-task-table-scroll"
            ref={virtualTasks.containerRef}
            onScroll={virtualTasks.onScroll}
          >
            <table className="run-detail-table">
              <RunDetailTableColGroup />
              <thead>
                <tr>
                  <th>Task</th>
                  <th>Status</th>
                  <th>Progress</th>
                  <th>Started</th>
                  <th>Finished</th>
                  <th>Duration</th>
                  <th>Output</th>
                </tr>
              </thead>
              <tbody>
                {virtualTasks.paddingTop > 0 && (
                  <tr className="virtual-spacer-row" aria-hidden="true">
                    <td colSpan={7} style={{ height: `${virtualTasks.paddingTop}px` }} />
                  </tr>
                )}
                {visibleTasks.map((task) => {
                  const isSelected = task.name === selectedTask;
                  const isTerminal = TERMINAL_TASK_STATUSES.has(task.status.toLowerCase());
                  const progress = isTerminal ? null : normalizeProgress(task.progress);
                  return (
                    <tr
                      key={task.id}
                      style={{
                        cursor: 'pointer',
                        background: isSelected ? 'var(--gantt-selected-bg)' : undefined,
                        outline: isSelected ? `1px solid var(--interactive-border-soft)` : undefined,
                      }}
                      onClick={() => onTaskSelect(isSelected ? null : task.name)}
                    >
                      <td style={{ fontWeight: isSelected ? 600 : undefined, color: isSelected ? 'var(--accent)' : undefined }}>
                        <span className="run-detail-cell-text" title={task.name}>{task.name}</span>
                      </td>
                      <td>
                        <StatusBadge status={task.status} />
                      </td>
                      <td>
                        {progress ? (
                          <div className="run-progress-cell">
                            <div className="run-progress-track">
                              <div className="run-progress-fill" style={{ width: `${progress.percent}%` }} />
                            </div>
                            <span>{`${progress.current}/${progress.total}`}</span>
                          </div>
                        ) : isTerminal ? (
                          <span style={{ color: 'var(--muted)' }}>done</span>
                        ) : (
                          '—'
                        )}
                      </td>
                      <td><span className="run-detail-cell-text">{task.started_at ? fmtDateTime(task.started_at) : '—'}</span></td>
                      <td><span className="run-detail-cell-text">{task.finished_at ? fmtDateTime(task.finished_at) : '—'}</span></td>
                      <td><span className="run-detail-cell-text">{task.started_at && task.finished_at ? fmtDuration(task.started_at, task.finished_at) : '—'}</span></td>
                      <td>
                        <code className="run-detail-cell-code" title={JSON.stringify(task.output ?? {})}>
                          {truncate(JSON.stringify(task.output ?? {}), 120)}
                        </code>
                      </td>
                    </tr>
                  );
                })}
                {virtualTasks.paddingBottom > 0 && (
                  <tr className="virtual-spacer-row" aria-hidden="true">
                    <td colSpan={7} style={{ height: `${virtualTasks.paddingBottom}px` }} />
                  </tr>
                )}
              </tbody>
            </table>
          </div>

          <div className="run-detail-log-list">
            {logsRequireTaskSelection ? (
              <p className="grid-run-detail-placeholder" style={{ minHeight: '120px' }}>
                Select a task to view logs for large runs.
              </p>
            ) : (
              logTasks.map((task) => (
                <LogBlock key={task.id} taskName={task.name} logs={task.logs} showDetails={logDetails} />
              ))
            )}
          </div>

          {selectedTaskDef?.params && Object.keys(selectedTaskDef.params).length > 0 && (
            <div className="log-block">
              <div className="log-title">{selectedTaskDef.name} definition</div>
              {typeof selectedTaskDef.params.condition_expr === 'string' && (
                <div style={{ marginBottom: '8px', padding: '6px 10px', borderRadius: '6px', background: 'var(--badge-skip-bg)', border: '1px solid var(--badge-skip-border)' }}>
                  <span style={{ color: 'var(--badge-skip-text)', fontWeight: 600, fontSize: '11px', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Condition (if):</span>
                  <code style={{ marginLeft: '8px', color: 'var(--badge-skip-text)' }}>{selectedTaskDef.params.condition_expr as string}</code>
                </div>
              )}
              <table style={{ whiteSpace: 'nowrap' }}>
                <tbody>
                  {Object.entries(selectedTaskDef.params)
                    .filter(([key]) => key !== 'condition_expr')
                    .map(([key, value]) => (
                      <tr key={key}>
                        <td style={{ color: 'var(--muted)', paddingRight: '16px' }}>{key}</td>
                        <td><code>{typeof value === 'string' ? value : JSON.stringify(value)}</code></td>
                      </tr>
                    ))}
                </tbody>
              </table>
            </div>
          )}

          {selectedTaskDef?.last_state && (
            <div className="log-block">
              <div className="log-title">{selectedTaskDef.name} last materialization</div>
              <table style={{ whiteSpace: 'nowrap' }}>
                <tbody>
                  <tr>
                    <td style={{ color: 'var(--muted)', paddingRight: '16px' }}>Status</td>
                    <td>
                      <StatusBadge status={selectedTaskDef.last_state.status} />
                    </td>
                  </tr>
                  <tr>
                    <td style={{ color: 'var(--muted)', paddingRight: '16px' }}>Last run</td>
                    <td><code>{selectedTaskDef.last_state.run_id}</code></td>
                  </tr>
                  <tr>
                    <td style={{ color: 'var(--muted)', paddingRight: '16px' }}>Finished</td>
                    <td>{selectedTaskDef.last_state.finished_at ? fmtDateTime(selectedTaskDef.last_state.finished_at) : '—'}</td>
                  </tr>
                  <tr>
                    <td style={{ color: 'var(--muted)', paddingRight: '16px' }}>Success at</td>
                    <td>{selectedTaskDef.last_state.success_at ? fmtDateTime(selectedTaskDef.last_state.success_at) : '—'}</td>
                  </tr>
                  <tr>
                    <td style={{ color: 'var(--muted)', paddingRight: '16px' }}>Failure at</td>
                    <td>{selectedTaskDef.last_state.failure_at ? fmtDateTime(selectedTaskDef.last_state.failure_at) : '—'}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
    </>
  );
}
