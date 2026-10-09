import type { Run } from '../types';
import { fmtRunOccurredAt, fmtRunOccurredAtLabel, truncate } from '../utils';
import { StatusBadge } from './StatusIcon';

interface WorkflowRunsListProps {
  workflowName: string;
  runs: Run[];
  loading: boolean;
  selectedRun: string;
  onRunClick: (runId: string) => void;
}

export default function WorkflowRunsList({
  workflowName,
  runs,
  loading,
  selectedRun,
  onRunClick,
}: WorkflowRunsListProps) {
  if (!workflowName) return null;

  return (
    <div className="workflow-runs-inline">
      <div className="panel-header">
        <h2>Runs</h2>
        <span className="pill">{runs.length}</span>
      </div>

      {loading ? (
        <p style={{ color: 'var(--muted)' }}>Loading runs...</p>
      ) : runs.length === 0 ? (
        <p style={{ color: 'var(--muted)' }}>No runs found for this workflow.</p>
      ) : (
        <div className="workflow-runs-inline-list">
          {runs.map((run) => (
            <button
              key={run.id}
              type="button"
              className={selectedRun === run.id ? 'workflow-runs-inline-item selected' : 'workflow-runs-inline-item'}
              onClick={() => onRunClick(run.id)}
              title={`${run.id} • ${fmtRunOccurredAtLabel(run)}`}
            >
              <span className="workflow-runs-inline-id">{truncate(run.id, 18)}</span>
              <StatusBadge status={run.status} />
              <span className="workflow-runs-inline-time">{fmtRunOccurredAt(run)}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
