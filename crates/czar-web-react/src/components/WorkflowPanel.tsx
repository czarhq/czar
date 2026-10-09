import { useMemo, useState, useRef, useEffect } from 'react';
import type { Run, Workflow } from '../types';
import { fmtDateTime, fmtRelativeTime, fmtRunOccurredAt, fmtRunOccurredAtLabel, getRunOccurredAt } from '../utils';
import { StatusBadge, StatusIcon } from './StatusIcon';
import Pagination from './Pagination';

const COMPACT_THRESHOLD = 220;

interface WorkflowPanelProps {
  workflows: Workflow[];
  selectedWorkflow: string;
  latestRunByWorkflow: Record<string, Run | null>;
  workflowSearchInput: string;
  workflowsTotal: number;
  workflowsPage: number;
  workflowsTotalPages: number;
  onWorkflowSearchInput: (value: string) => void;
  onWorkflowClick: (workflowName: string) => void;
  onWorkflowPageChange: (page: number) => void;
}

export default function WorkflowPanel({
  workflows,
  selectedWorkflow,
  latestRunByWorkflow,
  workflowSearchInput,
  workflowsTotal,
  workflowsPage,
  workflowsTotalPages,
  onWorkflowSearchInput,
  onWorkflowClick,
  onWorkflowPageChange,
}: WorkflowPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const [compact, setCompact] = useState(false);
  useEffect(() => {
    const el = panelRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      setCompact(entry.contentRect.width < COMPACT_THRESHOLD);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const [filtersOpen, setFiltersOpen] = useState(false);
  const [statusFilter, setStatusFilter] = useState('');
  const [dateFilter, setDateFilter] = useState('');
  const [sortBy, setSortBy] = useState<'date_desc' | 'date_asc' | 'status_asc' | 'status_desc'>('date_desc');
  const hasActiveFilters = statusFilter !== '' || dateFilter !== '' || sortBy !== 'date_desc';

  const visibleWorkflows = useMemo(() => {
    const nowMs = Date.now();
    const byDateRange = (run: Run | null): boolean => {
      if (!dateFilter) return true;
      if (!run) return false;
      const createdMs = new Date(run.created_at).getTime();
      if (!Number.isFinite(createdMs)) return false;
      if (dateFilter === '24h') return createdMs >= nowMs - 24 * 60 * 60 * 1000;
      if (dateFilter === '7d') return createdMs >= nowMs - 7 * 24 * 60 * 60 * 1000;
      if (dateFilter === '30d') return createdMs >= nowMs - 30 * 24 * 60 * 60 * 1000;
      if (dateFilter === '90d') return createdMs >= nowMs - 90 * 24 * 60 * 60 * 1000;
      return true;
    };

    const filtered = workflows.filter((workflow) => {
      const latest = latestRunByWorkflow[workflow.name] ?? null;
      if (statusFilter === 'never') return latest === null;
      if (statusFilter && statusFilter !== 'never') {
        if (!latest || latest.status.toLowerCase() !== statusFilter) return false;
      }
      return byDateRange(latest);
    });

    filtered.sort((a, b) => {
      const aRun = latestRunByWorkflow[a.name] ?? null;
      const bRun = latestRunByWorkflow[b.name] ?? null;
      const aDate = aRun ? new Date(aRun.created_at).getTime() : Number.NaN;
      const bDate = bRun ? new Date(bRun.created_at).getTime() : Number.NaN;
      const aStatus = (aRun?.status ?? '').toLowerCase();
      const bStatus = (bRun?.status ?? '').toLowerCase();

      if (sortBy === 'date_desc') {
        if (!Number.isFinite(aDate) && !Number.isFinite(bDate)) return a.name.localeCompare(b.name);
        if (!Number.isFinite(aDate)) return 1;
        if (!Number.isFinite(bDate)) return -1;
        return bDate - aDate;
      }
      if (sortBy === 'date_asc') {
        if (!Number.isFinite(aDate) && !Number.isFinite(bDate)) return a.name.localeCompare(b.name);
        if (!Number.isFinite(aDate)) return 1;
        if (!Number.isFinite(bDate)) return -1;
        return aDate - bDate;
      }
      if (sortBy === 'status_desc') {
        const statusCmp = bStatus.localeCompare(aStatus);
        if (statusCmp !== 0) return statusCmp;
      } else {
        const statusCmp = aStatus.localeCompare(bStatus);
        if (statusCmp !== 0) return statusCmp;
      }
      if (!Number.isFinite(aDate) && !Number.isFinite(bDate)) return a.name.localeCompare(b.name);
      if (!Number.isFinite(aDate)) return 1;
      if (!Number.isFinite(bDate)) return -1;
      return bDate - aDate;
    });

    return filtered;
  }, [workflows, latestRunByWorkflow, statusFilter, dateFilter, sortBy]);

  return (
    <div ref={panelRef} className="panel sidebar-panel">
      <div style={{ padding: '8px 12px', borderBottom: '1px solid var(--border)' }}>
        <input
          type="text"
          placeholder={`Search ${workflowsTotal} workflows...`}
          value={workflowSearchInput}
          onChange={(e) => onWorkflowSearchInput(e.target.value)}
          style={{ width: '100%', padding: '6px 8px', background: 'var(--panel)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: '4px', fontFamily: 'inherit' }}
        />
      </div>
      <div style={{ padding: '4px 12px', borderBottom: filtersOpen ? 'none' : '1px solid var(--border)' }}>
        <button
          type="button"
          className={`secondary subtle-control${filtersOpen ? ' active' : ''}`}
          style={{ gap: '6px' }}
          onClick={() => setFiltersOpen((v) => !v)}
          title="Toggle workflow filters"
          aria-label="Toggle workflow filters"
        >
          <span>filters</span>
          {hasActiveFilters && <span className="subtle-control-dot" aria-hidden="true">●</span>}
          <span aria-hidden="true">{filtersOpen ? '▴' : '▾'}</span>
        </button>
      </div>
      {filtersOpen && (
        <div style={{ padding: '8px 12px', borderBottom: '1px solid var(--border)', display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
          <select
            className="filter-select"
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            style={{ flex: '1 1 140px', minWidth: 0 }}
          >
            <option value="">All statuses</option>
            <option value="never">Never ran</option>
            <option value="pending">Pending</option>
            <option value="running">Running</option>
            <option value="paused">Paused</option>
            <option value="success">Success</option>
            <option value="failed">Failed</option>
            <option value="cancelled">Cancelled</option>
          </select>
          <select
            className="filter-select"
            value={dateFilter}
            onChange={(e) => setDateFilter(e.target.value)}
            style={{ flex: '1 1 140px', minWidth: 0 }}
          >
            <option value="">Any run date</option>
            <option value="24h">Last 24 hours</option>
            <option value="7d">Last 7 days</option>
            <option value="30d">Last 30 days</option>
            <option value="90d">Last 90 days</option>
          </select>
          <select
            className="filter-select"
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as 'date_desc' | 'date_asc' | 'status_asc' | 'status_desc')}
            style={{ flex: '1 1 160px', minWidth: 0 }}
          >
            <option value="date_desc">Sort: Latest run first</option>
            <option value="date_asc">Sort: Oldest run first</option>
            <option value="status_asc">Sort: Status A-Z</option>
            <option value="status_desc">Sort: Status Z-A</option>
          </select>
        </div>
      )}
      <div className="list">
        {visibleWorkflows.length === 0 ? (
          <p style={{ color: 'var(--muted)', padding: '12px' }}>No workflows found.</p>
        ) : (
          visibleWorkflows.map((workflow) => {
            const latest = latestRunByWorkflow[workflow.name] ?? null;
            const nextScheduleLabel = workflow.schedule_enabled && workflow.next_scheduled_at
              ? `Next: ${fmtDateTime(workflow.next_scheduled_at)}`
              : null;
            return (
              <div
                key={workflow.name}
                data-testid={`workflow-item-${workflow.name}`}
                className={selectedWorkflow === workflow.name ? 'item workflow-item selected' : 'item workflow-item'}
                onClick={() => onWorkflowClick(workflow.name)}
              >
                <div className="title workflow-item-name">{workflow.name}</div>
                {compact ? (
                  <div className="meta">
                    {latest ? (
                      <>
                        <StatusIcon status={latest.status} size={13} />
                        <span
                          style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', flex: 1, minWidth: 0 }}
                          title={[fmtRunOccurredAtLabel(latest), nextScheduleLabel].filter(Boolean).join(' • ')}
                        >
                          {fmtRelativeTime(getRunOccurredAt(latest))}
                        </span>
                      </>
                    ) : (
                      <span style={{ color: 'var(--muted)' }}>Never ran</span>
                    )}
                  </div>
                ) : (
                  <>
                    <div className="workflow-item-primary">
                      {latest ? (
                        <>
                          <StatusBadge status={latest.status} />
                          <span
                            className="text-sm workflow-item-time"
                            title={[fmtRunOccurredAtLabel(latest), nextScheduleLabel].filter(Boolean).join(' • ')}
                          >
                            Last: {fmtRunOccurredAt(latest)}
                          </span>
                        </>
                      ) : (
                        <>
                          <span className="text-sm workflow-item-status workflow-item-status--empty">Never ran</span>
                          <span className="workflow-item-time workflow-item-time--empty" aria-hidden="true" />
                        </>
                      )}
                    </div>
                    <div className="workflow-item-secondary">
                      {workflow.schedule && (
                        <span className="badge workflow-item-badge">{workflow.schedule}</span>
                      )}
                      {workflow.schedule_enabled && workflow.next_scheduled_at && (
                        <span
                          className="workflow-item-next"
                          title={`Next: ${fmtDateTime(workflow.next_scheduled_at)}`}
                        >
                          Next: {fmtDateTime(workflow.next_scheduled_at)}
                        </span>
                      )}
                    </div>
                  </>
                )}
              </div>
            );
          })
        )}
      </div>
      <Pagination
        currentPage={workflowsPage}
        totalPages={workflowsTotalPages}
        onPageChange={onWorkflowPageChange}
      />
    </div>
  );
}
