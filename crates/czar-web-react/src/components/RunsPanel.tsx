import { useState, useEffect, useRef } from 'react';
import type { Run } from '../types';
import { fmtDateTime, fmtRelativeTime, fmtRunOccurredAtLabel, getRunOccurredAt, truncate } from '../utils';
import { StatusIcon, StatusBadge } from './StatusIcon';
import Pagination from './Pagination';

const COMPACT_THRESHOLD = 220;

interface MultiSelectProps {
  options: string[];
  selected: string[];
  onChange: (selected: string[]) => void;
  placeholder: string;
  style?: React.CSSProperties;
}

function MultiSelect({ options, selected, onChange, placeholder, style }: MultiSelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, []);

  const label = selected.length === 0
    ? placeholder
    : selected.length === options.length
    ? 'All'
    : selected.length === 1
    ? selected[0]
    : `${selected.length} selected`;

  function toggle(option: string) {
    if (selected.includes(option)) {
      onChange(selected.filter(s => s !== option));
    } else {
      onChange([...selected, option]);
    }
  }

  const btnStyle: React.CSSProperties = {
    background: 'none',
    border: 'none',
    boxShadow: 'none',
    transform: 'none',
    padding: '2px 6px',
    fontSize: '11px',
    color: 'var(--accent)',
    cursor: 'pointer',
    borderRadius: '3px',
    fontWeight: 500,
  };

  return (
    <div ref={ref} style={{ position: 'relative', flex: '1 1 0', minWidth: 0, ...style }}>
      <button
        type="button"
        onClick={() => setOpen(o => !o)}
        style={{
          width: '100%',
          padding: '6px 8px',
          background: 'var(--panel)',
          color: selected.length > 0 ? 'var(--text)' : 'var(--muted)',
          border: '1px solid var(--border)',
          borderRadius: '4px',
          fontFamily: 'inherit',
          fontSize: '13px',
          textAlign: 'left',
          cursor: 'pointer',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          gap: '4px',
          boxShadow: 'none',
          transform: 'none',
        }}
      >
        <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{label}</span>
        <span style={{ color: 'var(--muted)', flexShrink: 0 }}>▾</span>
      </button>
      {open && (
        <div style={{
          position: 'absolute',
          top: '100%',
          left: 0,
          right: 0,
          minWidth: '180px',
          background: 'var(--panel)',
          border: '1px solid var(--border)',
          borderRadius: '4px',
          zIndex: 100,
          marginTop: '2px',
          boxShadow: 'var(--shadow)',
        }}>
          <div style={{ display: 'flex', gap: '2px', padding: '4px 6px', borderBottom: '1px solid var(--border)' }}>
            <button type="button" style={btnStyle} onClick={() => onChange(options)}>All</button>
            <button type="button" style={btnStyle} onClick={() => onChange([])}>None</button>
          </div>
          <div style={{ maxHeight: '280px', overflowY: 'auto' }}>
            {options.map(option => (
              <label
                key={option}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                  padding: '4px 8px',
                  cursor: 'pointer',
                  color: 'var(--text)',
                  fontSize: '13px',
                }}
                onMouseEnter={e => (e.currentTarget.style.background = 'var(--interactive-bg-hover)')}
                onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
              >
                <input
                  type="checkbox"
                  checked={selected.includes(option)}
                  onChange={() => toggle(option)}
                  style={{ cursor: 'pointer', margin: 0 }}
                />
                {option}
              </label>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

interface RunsPanelProps {
  runs: Run[];
  runsTotal: number;
  runsSearchInput: string;
  selectedRun: string;
  runsStatusFilter: string[];
  runsWorkflowFilter: string[];
  runsDateFilter: { from: string; to: string };
  workflowSuggestions: string[];
  runsPage: number;
  runsTotalPages: number;
  onRunsSearchInput: (value: string) => void;
  onRunsStatusFilterChange: (status: string[]) => void;
  onRunsWorkflowFilterChange: (workflow: string[]) => void;
  onRunsDateFilterChange: (range: { from: string; to: string }) => void;
  onRunClick: (runId: string) => void;
  onRunsPageChange: (page: number) => void;
}

export default function RunsPanel({
  runs,
  runsTotal,
  runsSearchInput,
  selectedRun,
  runsStatusFilter,
  runsDateFilter,
  runsWorkflowFilter,
  workflowSuggestions,
  runsPage,
  runsTotalPages,
  onRunsSearchInput,
  onRunsStatusFilterChange,
  onRunsWorkflowFilterChange,
  onRunsDateFilterChange,
  onRunClick,
  onRunsPageChange,
}: RunsPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const [compact, setCompact] = useState(false);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const hasActiveFilters = runsStatusFilter.length > 0 || runsWorkflowFilter.length > 0 || runsDateFilter.from !== '' || runsDateFilter.to !== '';

  useEffect(() => {
    const el = panelRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      setCompact(entry.contentRect.width < COMPACT_THRESHOLD);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  return (
    <div ref={panelRef} className="panel sidebar-panel">
      <div style={{ padding: '8px 12px', borderBottom: '1px solid var(--border)' }}>
        <input
          type="text"
          placeholder={`Search ${runsTotal} runs...`}
          value={runsSearchInput}
          onChange={(e) => onRunsSearchInput(e.target.value)}
          style={{ width: '100%', padding: '6px 8px', background: 'var(--panel)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: '4px', fontFamily: 'inherit' }}
        />
      </div>
      <div style={{ padding: '4px 12px', display: 'flex', justifyContent: 'flex-start', borderBottom: filtersOpen ? 'none' : '1px solid var(--border)' }}>
        <button
          type="button"
          className={`secondary subtle-control${filtersOpen ? ' active' : ''}`}
          style={{ gap: '6px' }}
          onClick={() => setFiltersOpen((v) => !v)}
          title="Toggle run filters"
          aria-label="Toggle run filters"
        >
          <span>filters</span>
          {hasActiveFilters && <span className="subtle-control-dot" aria-hidden="true">●</span>}
          <span aria-hidden="true">{filtersOpen ? '▴' : '▾'}</span>
        </button>
      </div>
      {filtersOpen && (
        <div style={{ padding: '8px 12px', borderBottom: '1px solid var(--border)', display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
          <MultiSelect
            options={['pending', 'running', 'paused', 'success', 'failed', 'cancelled']}
            selected={runsStatusFilter}
            onChange={onRunsStatusFilterChange}
            placeholder="All statuses"
            style={{ flex: compact ? '1 1 100%' : '1 1 120px' }}
          />
          <MultiSelect
            options={workflowSuggestions}
            selected={runsWorkflowFilter}
            onChange={onRunsWorkflowFilterChange}
            placeholder="All workflows"
            style={{ flex: compact ? '1 1 100%' : '1 1 120px' }}
          />
          <div style={{ display: 'flex', flex: '1 1 100%', gap: '8px', alignItems: 'center' }}>
            <input
              type="date"
              value={runsDateFilter.from}
              onChange={(e) => onRunsDateFilterChange({ ...runsDateFilter, from: e.target.value })}
              style={{ flex: 1, minWidth: 0, padding: '6px 8px', background: 'var(--panel)', color: runsDateFilter.from ? 'var(--text)' : 'var(--muted)', border: '1px solid var(--border)', borderRadius: '4px', fontFamily: 'inherit', fontSize: 'inherit', colorScheme: 'dark' }}
            />
            <span style={{ color: 'var(--muted)', flexShrink: 0, fontSize: '12px' }}>to</span>
            <input
              type="date"
              value={runsDateFilter.to}
              onChange={(e) => onRunsDateFilterChange({ ...runsDateFilter, to: e.target.value })}
              style={{ flex: 1, minWidth: 0, padding: '6px 8px', background: 'var(--panel)', color: runsDateFilter.to ? 'var(--text)' : 'var(--muted)', border: '1px solid var(--border)', borderRadius: '4px', fontFamily: 'inherit', fontSize: 'inherit', colorScheme: 'dark' }}
            />
          </div>
        </div>
      )}
      <div className="list large">
        {runs.length === 0 ? (
          <p style={{ color: 'var(--muted)', padding: '12px' }}>No runs yet.</p>
        ) : (
          runs.map((run) => (
            <div
              key={run.id}
              data-testid={`run-item-${run.id}`}
              data-run-status={run.status}
              className={selectedRun === run.id ? 'item selected' : 'item'}
              onClick={() => onRunClick(run.id)}
              title={fmtRunOccurredAtLabel(run)}
            >
              <div className="title">
                {truncate(run.workflow, 32)}
              </div>
              <div className="meta">
                {compact
                  ? <StatusIcon status={run.status} size={13} />
                  : <StatusBadge status={run.status} />
                }
                <span className="text-sm" style={{ color: 'var(--muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', flex: 1, minWidth: 0 }}>
                  {compact ? fmtRelativeTime(getRunOccurredAt(run)) : `${fmtDateTime(getRunOccurredAt(run))} · ${run.id}`}
                </span>
              </div>
            </div>
          ))
        )}
      </div>
      <Pagination currentPage={runsPage} totalPages={runsTotalPages} onPageChange={onRunsPageChange} />
    </div>
  );
}
