import { useMemo, useState } from 'react';
import { fmtDateTime, statusClass } from '../utils';
import { useVirtualRows } from '../hooks/useVirtualRows';

export interface TaskMaterializationRow {
  task_name: string;
  last_status?: string;
  finished_at?: string;
  success_at?: string;
  failure_at?: string;
}

interface TaskMaterializationPanelProps {
  rows: TaskMaterializationRow[];
  onTaskFocus: (taskName: string) => void;
}

const MATERIALIZATION_VIRTUAL_THRESHOLD = 120;
const MATERIALIZATION_ROW_HEIGHT = 41;

export default function TaskMaterializationPanel({
  rows,
  onTaskFocus,
}: TaskMaterializationPanelProps) {
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState('all');

  const statusOptions = useMemo(() => {
    const statuses = new Set<string>();
    rows.forEach((row) => {
      if (row.last_status) {
        statuses.add(row.last_status);
      }
    });
    return ['all', 'never', ...[...statuses].sort()];
  }, [rows]);

  const filteredRows = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return rows.filter((row) => {
      if (needle && !row.task_name.toLowerCase().includes(needle)) {
        return false;
      }
      if (statusFilter === 'all') return true;
      if (statusFilter === 'never') return !row.last_status;
      return row.last_status === statusFilter;
    });
  }, [rows, search, statusFilter]);
  const virtualRows = useVirtualRows<HTMLDivElement>({
    count: filteredRows.length,
    rowHeight: MATERIALIZATION_ROW_HEIGHT,
    enabled: filteredRows.length > MATERIALIZATION_VIRTUAL_THRESHOLD,
  });
  const visibleRows = filteredRows.slice(virtualRows.start, virtualRows.end);

  return (
    <div>
      <div className="grid-run-detail-heading">
        <h3>Task Last Materialization</h3>
      </div>
      <div className="materialization-filters">
        <label className="execution-inline-label">
          Search
          <input
            type="search"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Task name"
          />
        </label>
        <label className="execution-inline-label">
          Last State
          <select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}>
            {statusOptions.map((status) => (
              <option key={status} value={status}>
                {status === 'all' ? 'All' : status === 'never' ? 'Never' : status}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div
        className="materialization-table-scroll"
        ref={virtualRows.containerRef}
        onScroll={virtualRows.onScroll}
      >
        <table>
          <thead>
            <tr>
              <th>Task</th>
              <th>Last Status</th>
              <th>Last Finished</th>
              <th>Last Success</th>
              <th>Last Failure</th>
            </tr>
          </thead>
          <tbody>
            {virtualRows.paddingTop > 0 && (
              <tr className="virtual-spacer-row" aria-hidden="true">
                <td colSpan={5} style={{ height: `${virtualRows.paddingTop}px` }} />
              </tr>
            )}
            {visibleRows.map((row) => (
              <tr key={row.task_name}>
                <td>
                  <button
                    type="button"
                    className="link-button"
                    title="Focus task in graph"
                    onClick={() => onTaskFocus(row.task_name)}
                  >
                    {row.task_name}
                  </button>
                </td>
                <td>
                  {row.last_status ? (
                    <span className={`badge ${statusClass(row.last_status)}`}>{row.last_status}</span>
                  ) : (
                    <span style={{ color: 'var(--muted)' }}>never</span>
                  )}
                </td>
                <td>{row.finished_at ? fmtDateTime(row.finished_at) : '—'}</td>
                <td>{row.success_at ? fmtDateTime(row.success_at) : '—'}</td>
                <td>{row.failure_at ? fmtDateTime(row.failure_at) : '—'}</td>
              </tr>
            ))}
            {virtualRows.paddingBottom > 0 && (
              <tr className="virtual-spacer-row" aria-hidden="true">
                <td colSpan={5} style={{ height: `${virtualRows.paddingBottom}px` }} />
              </tr>
            )}
            {filteredRows.length === 0 && (
              <tr>
                <td colSpan={5} style={{ color: 'var(--muted)' }}>
                  No tasks match the current filter.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
