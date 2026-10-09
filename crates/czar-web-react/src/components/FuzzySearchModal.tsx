import { useEffect, useRef, useState, useCallback } from 'react';
import type { Run, Workflow } from '../types';
import { fmtDateTime } from '../utils';
import { StatusBadge } from './StatusIcon';
import { fetchRuns, fetchWorkflows } from '../api';

interface FuzzySearchModalProps {
  runs: Run[];
  workflows: Workflow[];
  onSelectRun: (runId: string) => void;
  onSelectWorkflow: (name: string) => void;
  onClose: () => void;
}

type SearchResult =
  | { type: 'workflow'; workflow: Workflow }
  | { type: 'run'; run: Run };

function fuzzyMatch(text: string, query: string): boolean {
  if (!query) return true;
  const t = text.toLowerCase();
  const q = query.toLowerCase();
  let qi = 0;
  for (const c of t) {
    if (qi < q.length && c === q[qi]) qi++;
  }
  return qi === q.length;
}

/** Returns indices of matched characters (greedy left-to-right). */
function fuzzyMatchPositions(text: string, query: string): number[] | null {
  if (!query) return [];
  const t = text.toLowerCase();
  const q = query.toLowerCase();
  const positions: number[] = [];
  let qi = 0;
  for (let i = 0; i < t.length; i++) {
    if (qi < q.length && t[i] === q[qi]) {
      positions.push(i);
      qi++;
    }
  }
  return qi === q.length ? positions : null;
}

function HighlightedText({ text, query }: { text: string; query: string }) {
  const positions = fuzzyMatchPositions(text, query);
  if (!positions || positions.length === 0) {
    return <span>{text}</span>;
  }
  const matched = new Set(positions);
  const parts: React.ReactNode[] = [];
  let i = 0;
  while (i < text.length) {
    if (matched.has(i)) {
      let j = i;
      while (j < text.length && matched.has(j)) j++;
      parts.push(
        <mark key={i} style={{ background: 'transparent', color: 'var(--accent)', fontWeight: 700 }}>
          {text.slice(i, j)}
        </mark>
      );
      i = j;
    } else {
      let j = i;
      while (j < text.length && !matched.has(j)) j++;
      parts.push(<span key={i}>{text.slice(i, j)}</span>);
      i = j;
    }
  }
  return <>{parts}</>;
}

const MAX_RESULTS = 20;
const SERVER_SEARCH_LIMIT = 50;
const DEBOUNCE_MS = 300;

function searchLocal(runs: Run[], workflows: Workflow[], query: string): SearchResult[] {
  const results: SearchResult[] = [];
  for (const wf of workflows) {
    if (fuzzyMatch(wf.name, query)) results.push({ type: 'workflow', workflow: wf });
  }
  for (const run of runs) {
    if (
      fuzzyMatch(run.workflow, query) ||
      fuzzyMatch(run.id.substring(0, 12), query) ||
      fuzzyMatch(fmtDateTime(run.created_at), query) ||
      fuzzyMatch(run.status, query)
    ) {
      results.push({ type: 'run', run });
    }
  }
  return results.slice(0, MAX_RESULTS);
}

export default function FuzzySearchModal({ runs, workflows, onSelectRun, onSelectWorkflow, onClose }: FuzzySearchModalProps) {
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState(0);
  const [serverResults, setServerResults] = useState<SearchResult[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const localResults = query
    ? searchLocal(runs, workflows, query)
    : [
        ...workflows.slice(0, 5).map((workflow): SearchResult => ({ type: 'workflow', workflow })),
        ...runs.slice(0, MAX_RESULTS - Math.min(5, workflows.length)).map((run): SearchResult => ({ type: 'run', run })),
      ];

  // Merge local + server results, deduplicating by key
  const results: SearchResult[] = query ? (() => {
    const seen = new Set<string>();
    const merged: SearchResult[] = [];
    for (const r of [...localResults, ...serverResults]) {
      const key = r.type === 'run' ? `run:${r.run.id}` : `wf:${r.workflow.name}`;
      if (!seen.has(key)) { seen.add(key); merged.push(r); }
    }
    return merged.slice(0, MAX_RESULTS);
  })() : localResults;

  // Server-side search debounced
  useEffect(() => {
    if (!query) { setServerResults([]); return; }
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      try {
        const [wfResp, runsResp] = await Promise.all([
          fetchWorkflows(SERVER_SEARCH_LIMIT, 0, query),
          fetchRuns(SERVER_SEARCH_LIMIT, 0, query, [], []),
        ]);
        const sr: SearchResult[] = [
          ...wfResp.items.map((wf): SearchResult => ({ type: 'workflow', workflow: wf })),
          ...runsResp.items.map((run): SearchResult => ({ type: 'run', run })),
        ];
        setServerResults(sr);
      } catch {
        // silently ignore server errors
      }
    }, DEBOUNCE_MS);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [query]);

  // Reset selection when query changes
  useEffect(() => {
    setSelected(0);
  }, [query]);

  // Scroll selected item into view
  useEffect(() => {
    if (listRef.current) {
      const item = listRef.current.children[selected] as HTMLElement | undefined;
      item?.scrollIntoView({ block: 'nearest' });
    }
  }, [selected]);

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSelect = useCallback((result: SearchResult) => {
    if (result.type === 'run') onSelectRun(result.run.id);
    else onSelectWorkflow(result.workflow.name);
  }, [onSelectRun, onSelectWorkflow]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelected((s) => Math.min(s + 1, results.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelected((s) => Math.max(s - 1, 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const result = results[selected];
      if (result) handleSelect(result);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }, [results, selected, handleSelect, onClose]);

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 1000,
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'center',
        paddingTop: '10vh',
        background: 'var(--overlay-bg)',
        backdropFilter: 'blur(4px)',
      }}
      onClick={onClose}
    >
      <div
        style={{
          width: '100%',
          maxWidth: '640px',
          background: 'var(--panel-2)',
          border: '1px solid var(--accent-2)',
          borderRadius: '10px',
          boxShadow: 'var(--shadow)',
          overflow: 'hidden',
        }}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        {/* Search input */}
        <div style={{ display: 'flex', alignItems: 'center', padding: '12px 16px', borderBottom: '1px solid var(--border)', gap: '10px' }}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" style={{ flexShrink: 0, color: 'var(--muted)' }}>
            <circle cx="6.5" cy="6.5" r="5" stroke="currentColor" strokeWidth="1.5" />
            <path d="M10.5 10.5L14 14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search workflows and runs…"
            style={{
              flex: 1,
              background: 'transparent',
              border: 'none',
              outline: 'none',
              color: 'var(--text)',
              fontFamily: 'inherit',
              fontSize: '15px',
            }}
          />
          <kbd style={{ fontSize: '11px', color: 'var(--muted)', background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: '4px', padding: '2px 6px', fontFamily: 'inherit' }}>
            Esc
          </kbd>
        </div>

        {/* Results */}
        <div
          ref={listRef}
          style={{ maxHeight: '360px', overflowY: 'auto' }}
        >
          {results.length === 0 ? (
            <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--muted)', fontSize: '14px' }}>
              No results match <strong style={{ color: 'var(--text)' }}>{query}</strong>
            </div>
          ) : (
            results.map((result, idx) => (
              result.type === 'workflow' ? (
                <div
                  key={`wf:${result.workflow.name}`}
                  onClick={() => handleSelect(result)}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '10px',
                    padding: '9px 16px',
                    cursor: 'pointer',
                    background: idx === selected ? 'var(--panel)' : 'transparent',
                    borderLeft: idx === selected ? '2px solid var(--accent-2)' : '2px solid transparent',
                    transition: 'background 80ms',
                  }}
                  onMouseEnter={() => setSelected(idx)}
                >
                  <span style={{ fontSize: '10px', color: 'var(--accent-3)', background: 'var(--panel)', border: '1px solid var(--accent-3)', borderRadius: '3px', padding: '1px 5px', flexShrink: 0, fontWeight: 600 }}>
                    WF
                  </span>
                  <span style={{ flex: 1, fontSize: '14px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    <HighlightedText text={result.workflow.name} query={query} />
                  </span>
                  {result.workflow.schedule && (
                    <span style={{ fontSize: '12px', color: 'var(--muted)', flexShrink: 0 }}>
                      {result.workflow.schedule}
                    </span>
                  )}
                </div>
              ) : (
                <div
                  key={`run:${result.run.id}`}
                  onClick={() => handleSelect(result)}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '10px',
                    padding: '9px 16px',
                    cursor: 'pointer',
                    background: idx === selected ? 'var(--panel)' : 'transparent',
                    borderLeft: idx === selected ? '2px solid var(--accent-2)' : '2px solid transparent',
                    transition: 'background 80ms',
                  }}
                  onMouseEnter={() => setSelected(idx)}
                >
                  <span style={{ fontFamily: 'monospace', fontSize: '12px', color: 'var(--muted)', flexShrink: 0, minWidth: '80px' }}>
                    <HighlightedText text={result.run.id.substring(0, 12)} query={query} />
                  </span>
                  <span style={{ flex: 1, fontSize: '14px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    <HighlightedText text={result.run.workflow} query={query} />
                  </span>
                  <span style={{ fontSize: '12px', color: 'var(--muted)', flexShrink: 0 }}>
                    <HighlightedText text={fmtDateTime(result.run.created_at)} query={query} />
                  </span>
                  <StatusBadge status={result.run.status} />
                </div>
              )
            ))
          )}
        </div>

        {/* Footer hint */}
        <div style={{ padding: '6px 16px', borderTop: '1px solid var(--border)', display: 'flex', gap: '16px', fontSize: '11px', color: 'var(--muted)' }}>
          <span><kbd style={{ background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: '3px', padding: '1px 5px', fontFamily: 'inherit' }}>↑↓</kbd> navigate</span>
          <span><kbd style={{ background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: '3px', padding: '1px 5px', fontFamily: 'inherit' }}>Enter</kbd> open</span>
          <span><kbd style={{ background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: '3px', padding: '1px 5px', fontFamily: 'inherit' }}>Ctrl+P</kbd> toggle</span>
        </div>
      </div>
    </div>
  );
}
