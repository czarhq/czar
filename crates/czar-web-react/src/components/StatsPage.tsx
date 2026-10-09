import { useEffect, useRef, useState, type ReactNode } from 'react';
import { fetchRunDetail, fetchRuns } from '../api';
import { StatusBadge } from './StatusIcon';
import type { Run, Task } from '../types';
import { fmtDuration } from '../utils';

const REFRESH_INTERVAL_MS = 15_000;
const RECENT_RUNS_LIMIT = 200;
const ACTIVE_RUNS_TASK_FETCH_LIMIT = 20;

const TIME_WINDOWS = [
  { label: '1m', ms: 60_000 },
  { label: '15m', ms: 15 * 60_000 },
  { label: '1h', ms: 60 * 60_000 },
  { label: '6h', ms: 6 * 60 * 60_000 },
  { label: '24h', ms: 24 * 60 * 60_000 },
];

const RUN_STATUSES = ['pending', 'running', 'paused', 'success', 'failed', 'cancelled'];
const TASK_STATUSES = ['pending', 'dispatched', 'running', 'paused', 'success', 'failed', 'cancelled'];

interface StatusCounts {
  [key: string]: number;
}

interface StatsData {
  runStatusCounts: StatusCounts;
  runTotals: StatusCounts;
  recentRuns: Run[];
  taskStatusCounts: StatusCounts;
  activeRunCount: number;
  activeTaskRunIds: string[];
  lastUpdated: Date;
}

function countByStatus(items: { status: string }[]): StatusCounts {
  const counts: StatusCounts = {};
  for (const item of items) {
    counts[item.status] = (counts[item.status] ?? 0) + 1;
  }
  return counts;
}

function countFinishedInWindow(runs: Run[], windowMs: number, status?: string): number {
  const cutoff = Date.now() - windowMs;
  return runs.filter((r) => {
    if (!r.finished_at) return false;
    if (new Date(r.finished_at).getTime() < cutoff) return false;
    if (status && r.status !== status) return false;
    return true;
  }).length;
}

function StatCard({ label, value, variant }: { label: string; value: number | string; variant?: 'ok' | 'warn' | 'danger' | 'muted' }) {
  const cls = `stats-stat-card${variant ? ` stats-stat-card--${variant}` : ''}`;
  return (
    <div className={cls}>
      <span className="stats-stat-value">{value}</span>
      <span className="stats-stat-label">{label}</span>
    </div>
  );
}

function SectionHeading({ children }: { children: ReactNode }) {
  return <h2 className="stats-section-heading">{children}</h2>;
}

function TimeWindowTable({
  runs,
  label,
}: {
  runs: Run[];
  label: string;
}) {
  const terminalStatuses = ['success', 'failed', 'cancelled'];
  return (
    <div className="stats-table-wrap">
      <table className="stats-table">
        <thead>
          <tr>
            <th>{label}</th>
            {TIME_WINDOWS.map((w) => <th key={w.label}>{w.label}</th>)}
          </tr>
        </thead>
        <tbody>
          {terminalStatuses.map((status) => (
            <tr key={status}>
              <td>
                <StatusBadge status={status} />
              </td>
              {TIME_WINDOWS.map((w) => (
                <td key={w.label} className="stats-table-num">
                  {countFinishedInWindow(runs, w.ms, status)}
                </td>
              ))}
            </tr>
          ))}
          <tr className="stats-table-total-row">
            <td>total finished</td>
            {TIME_WINDOWS.map((w) => (
              <td key={w.label} className="stats-table-num">
                {countFinishedInWindow(runs, w.ms)}
              </td>
            ))}
          </tr>
        </tbody>
      </table>
    </div>
  );
}

function TaskPhaseSummary({ tasks }: { tasks: Task[] }) {
  const counts = countByStatus(tasks);
  return (
    <div className="stats-task-phase-grid">
      {TASK_STATUSES.map((status) => {
        const n = counts[status] ?? 0;
        const variant =
          status === 'success' ? 'ok'
            : status === 'running' || status === 'dispatched' ? 'warn'
              : status === 'failed' ? 'danger'
                : n === 0 ? 'muted'
                  : undefined;
        return <StatCard key={status} label={status} value={n} variant={variant} />;
      })}
    </div>
  );
}

export default function StatsPage() {
  const [data, setData] = useState<StatsData | null>(null);
  const [activeTasks, setActiveTasks] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshNonceRef = useRef(0);

  const fetchData = async (nonce: number) => {
    try {
      // Fetch total counts by status (just need the `total` field, limit=1 each)
      const statusFetches = RUN_STATUSES.map((s) =>
        fetchRuns(1, 0, '', [], [s]).then((r) => ({ status: s, total: r.total }))
      );
      // Fetch recent runs for time-windowed stats
      const recentFetch = fetchRuns(RECENT_RUNS_LIMIT, 0, '', [], []);

      const [statusResults, recentResult] = await Promise.all([
        Promise.all(statusFetches),
        recentFetch,
      ]);

      if (nonce !== refreshNonceRef.current) return;

      const runTotals: StatusCounts = {};
      for (const { status, total } of statusResults) {
        runTotals[status] = total;
      }

      const activeRuns = recentResult.items.filter(
        (r) => r.status === 'running' || r.status === 'pending' || r.status === 'dispatched'
      );

      setData({
        runStatusCounts: countByStatus(recentResult.items),
        runTotals,
        recentRuns: recentResult.items,
        taskStatusCounts: {},
        activeRunCount: (runTotals.running ?? 0) + (runTotals.pending ?? 0),
        activeTaskRunIds: activeRuns.slice(0, ACTIVE_RUNS_TASK_FETCH_LIMIT).map((r) => r.id),
        lastUpdated: new Date(),
      });
      setError(null);
      setLoading(false);

      // Fetch task details for active runs
      const runIds = activeRuns.slice(0, ACTIVE_RUNS_TASK_FETCH_LIMIT).map((r) => r.id);
      if (runIds.length > 0) {
        const taskFetches = runIds.map((id) => fetchRunDetail(id).then((d) => d.tasks));
        const taskArrays = await Promise.allSettled(taskFetches);
        if (nonce !== refreshNonceRef.current) return;
        const allTasks = taskArrays.flatMap((r) => (r.status === 'fulfilled' ? r.value : []));
        setActiveTasks(allTasks);
      } else {
        setActiveTasks([]);
      }
    } catch (err) {
      if (nonce !== refreshNonceRef.current) return;
      setError(err instanceof Error ? err.message : 'Failed to load stats.');
      setLoading(false);
    }
  };

  useEffect(() => {
    setLoading(true);
    const nonce = ++refreshNonceRef.current;
    fetchData(nonce);

    const interval = setInterval(() => {
      const n = ++refreshNonceRef.current;
      fetchData(n);
    }, REFRESH_INTERVAL_MS);

    return () => {
      clearInterval(interval);
      refreshNonceRef.current++; // cancel any in-flight fetch
    };
  }, []);

  if (loading && !data) {
    return (
      <div className="stats-page">
        <p className="stats-loading">Loading stats…</p>
      </div>
    );
  }

  if (error && !data) {
    return (
      <div className="stats-page">
        <p className="stats-error">{error}</p>
      </div>
    );
  }

  if (!data) return null;

  const { runTotals, recentRuns, lastUpdated } = data;

  const activeStatuses = ['pending', 'running', 'paused'];
  const terminalStatuses = ['success', 'failed', 'cancelled'];

  return (
    <div className="stats-page">
      {error && (
        <p className="stats-error" style={{ marginBottom: '16px' }}>{error}</p>
      )}

      {/* Active Runs */}
      <SectionHeading>Active Runs</SectionHeading>
      <div className="stats-stat-row">
        {activeStatuses.map((status) => {
          const n = runTotals[status] ?? 0;
          const variant =
            status === 'running' ? 'warn'
              : n > 0 ? undefined
                : 'muted';
          return <StatCard key={status} label={status} value={n} variant={variant} />;
        })}
      </div>

      {/* All-time totals */}
      <SectionHeading>All-time Run Totals</SectionHeading>
      <div className="stats-stat-row">
        {terminalStatuses.map((status) => {
          const n = runTotals[status] ?? 0;
          const variant =
            status === 'success' ? (n > 0 ? 'ok' : 'muted')
              : status === 'failed' ? (n > 0 ? 'danger' : 'muted')
                : n > 0 ? undefined : 'muted';
          return <StatCard key={status} label={status} value={n} variant={variant} />;
        })}
        <StatCard
          label="total"
          value={Object.keys(runTotals).reduce((sum, k) => sum + (runTotals[k] ?? 0), 0)}
        />
      </div>

      {/* Time-windowed completion */}
      <SectionHeading>
        Runs Completed by Time Window
        <span className="stats-section-subtitle">
          based on last {RECENT_RUNS_LIMIT} fetched runs
        </span>
      </SectionHeading>
      <TimeWindowTable runs={recentRuns} label="Status" />

      {/* Active task breakdown */}
      {activeTasks.length > 0 && (
        <>
          <SectionHeading>
            Task Phases in Active Runs
            <span className="stats-section-subtitle">
              from up to {ACTIVE_RUNS_TASK_FETCH_LIMIT} currently active runs · {activeTasks.length} tasks
            </span>
          </SectionHeading>
          <TaskPhaseSummary tasks={activeTasks} />

          {/* Avg task duration for finished tasks */}
          <FinishedTaskDurations tasks={activeTasks} />
        </>
      )}

      {activeTasks.length === 0 && (runTotals.running ?? 0) === 0 && (runTotals.pending ?? 0) === 0 && (
        <div className="stats-no-active">
          No active runs.
        </div>
      )}

      <p className="stats-last-updated">
        Updated {lastUpdated.toLocaleTimeString()} · auto-refreshes every {REFRESH_INTERVAL_MS / 1000}s
      </p>
    </div>
  );
}

function FinishedTaskDurations({ tasks }: { tasks: Task[] }) {
  const finished = tasks.filter((t) => t.started_at && t.finished_at);
  if (finished.length === 0) return null;

  const durations = finished.map((t) => {
    const start = new Date(t.started_at!).getTime();
    const end = new Date(t.finished_at!).getTime();
    return end - start;
  });

  const avg = durations.reduce((a, b) => a + b, 0) / durations.length;
  const min = Math.min(...durations);
  const max = Math.max(...durations);

  const fmtMs = (ms: number) => {
    if (ms < 1000) return `${ms}ms`;
    // reuse fmtDuration by constructing fake date strings
    const now = new Date('2000-01-01T00:00:00Z');
    const end = new Date(now.getTime() + ms);
    return fmtDuration(now.toISOString(), end.toISOString());
  };

  return (
    <div className="stats-duration-row">
      <span className="stats-duration-label">Task duration (finished)</span>
      <span className="stats-duration-item">avg <strong>{fmtMs(avg)}</strong></span>
      <span className="stats-duration-item">min <strong>{fmtMs(min)}</strong></span>
      <span className="stats-duration-item">max <strong>{fmtMs(max)}</strong></span>
      <span className="stats-duration-item stats-duration-count">{finished.length} tasks</span>
    </div>
  );
}
