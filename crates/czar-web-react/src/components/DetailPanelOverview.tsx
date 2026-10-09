import { lazy, Suspense } from 'react';
import type { RunDetail, WorkflowInfo } from '../types';
import { fmtDateTime, fmtDuration } from '../utils';
import { StatusIcon } from './StatusIcon';

const WorkflowYamlPanel = lazy(() => import('./WorkflowYamlPanel'));

interface DetailPanelOverviewProps {
  selectedWorkflow: string;
  selectedRun: string;
  runDetail: RunDetail | null;
  workflowDetail: WorkflowInfo | null;
  sourceYaml?: string | null;
  onPauseRun: (runId: string) => void;
  onResumeRun: (runId: string) => void;
  onCancelRun: (runId: string) => void;
}

export default function DetailPanelOverview({
  selectedWorkflow,
  selectedRun,
  runDetail,
  workflowDetail,
  sourceYaml,
  onPauseRun,
  onResumeRun,
  onCancelRun,
}: DetailPanelOverviewProps) {
  if (!selectedWorkflow) {
    return (
      <div className="detail-overview" aria-hidden="true" style={{ visibility: 'hidden' }}>
        <div className="detail-overview-left">
          <h3 style={{ margin: '0 0 4px 0', fontSize: '17px', display: 'flex', alignItems: 'center', gap: '8px' }}>&nbsp;</h3>
        </div>
        <div className="detail-overview-right">
          <button className="secondary subtle-control" disabled style={{ textTransform: 'lowercase' }}>&nbsp;</button>
        </div>
      </div>
    );
  }

  const run = selectedRun ? (runDetail?.run ?? null) : null;
  const activeWorkflow = workflowDetail ?? runDetail?.workflow ?? null;
  const isRunLoading = Boolean(selectedRun) && !run;
  const isRunRefreshing = Boolean(selectedRun && run && run.id !== selectedRun);
  const selectionSummary = run?.task_selection
    ? `${run.task_selection.mode} • ${run.task_selection.resolved_tasks.length} task(s)`
    : null;

  return (
    <div className="detail-overview" data-testid="run-overview" data-run-status={run?.status ?? ''}>
      <div className="detail-overview-left">
        <h3 style={{ margin: '0 0 4px 0', fontSize: '17px', display: 'flex', alignItems: 'center', gap: '8px' }}>
          {run && <StatusIcon status={run.status} size={16} colored />}
          {run?.workflow ?? selectedWorkflow}
          {isRunRefreshing && <span className="pill">Updating…</span>}
        </h3>
        {run ? (
          <>
            <div className="meta">
              <span className="run-id">Run {run.id}</span>
              <span>Created: {fmtDateTime(run.created_at)}</span>
              <span>Started: {run.started_at ? fmtDateTime(run.started_at) : '—'}</span>
              <span>Finished: {run.finished_at ? fmtDateTime(run.finished_at) : '—'}</span>
              <span>Duration: {run.started_at && run.finished_at ? fmtDuration(run.started_at, run.finished_at) : '—'}</span>
              {selectionSummary && <span>Selection: {selectionSummary}</span>}
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginTop: '8px', flexWrap: 'wrap', minHeight: '30px' }}>
              {(run.status === 'pending' || run.status === 'running') && (
                <button style={{ padding: '4px 12px', background: 'var(--warm-bg-active)', border: '1px solid var(--warm-border)', borderRadius: '4px', cursor: 'pointer', color: 'var(--text)' }} onClick={() => onPauseRun(run.id)}>Pause</button>
              )}
              {run.status === 'paused' && (
                <button style={{ padding: '4px 12px', background: 'var(--badge-ok-bg)', border: '1px solid var(--badge-ok-border)', borderRadius: '4px', cursor: 'pointer', color: 'var(--badge-ok-text)' }} onClick={() => onResumeRun(run.id)}>Resume</button>
              )}
              {(run.status === 'pending' || run.status === 'running' || run.status === 'paused') && (
                <button style={{ padding: '4px 12px', background: 'var(--danger)', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer' }} onClick={() => onCancelRun(run.id)}>Cancel</button>
              )}
            </div>
          </>
        ) : isRunLoading ? (
          <>
            <div className="meta" style={{ visibility: 'hidden' }} aria-hidden="true">
              <span className="run-id">Run —————————————————</span>
              <span>Created: ——————————</span>
              <span>Started: ——————————</span>
              <span>Finished: ——————————</span>
              <span>Duration: ————</span>
            </div>
            <div style={{ minHeight: '30px', marginTop: '8px' }} aria-hidden="true" />
          </>
        ) : null}
        {!run && activeWorkflow && (
          <div className="meta">
            {activeWorkflow.schedule && <span>Schedule: {activeWorkflow.schedule}</span>}
            {activeWorkflow.schedule_enabled && activeWorkflow.next_scheduled_at && (
              <span>Next: {fmtDateTime(activeWorkflow.next_scheduled_at)}</span>
            )}
          </div>
        )}
      </div>
      <div className="detail-overview-right">
        <Suspense fallback={<button className="secondary subtle-control" disabled style={{ textTransform: 'lowercase' }}>yaml</button>}>
          <WorkflowYamlPanel sourceYaml={sourceYaml} />
        </Suspense>
      </div>
    </div>
  );
}
