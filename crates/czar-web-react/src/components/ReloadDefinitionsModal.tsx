import type { ReloadWorkflowDefinitionsResponse } from '../types';

interface ReloadDefinitionsModalProps {
  loading: boolean;
  result: ReloadWorkflowDefinitionsResponse | null;
  errorMessage: string | null;
  onClose: () => void;
}

function formatSummary(result: ReloadWorkflowDefinitionsResponse): string {
  const parts: string[] = [];
  if (result.skipped) parts.push(`${result.skipped} skipped`);
  if (result.failed) parts.push(`${result.failed} failed`);
  const suffix = parts.length > 0 ? ` (${parts.join(', ')})` : '';
  return `Reloaded ${result.reloaded} workflow definitions from ${result.discovered} discovered YAML files${suffix}.`;
}

export default function ReloadDefinitionsModal({
  loading,
  result,
  errorMessage,
  onClose,
}: ReloadDefinitionsModalProps) {
  const isOpen = loading || Boolean(result || errorMessage);
  if (!isOpen) return null;

  const failedRows = result?.results.filter((row) => row.outcome === 'failed') ?? [];
  const skippedRows = result?.results.filter((row) => row.outcome === 'skipped') ?? [];

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="panel run-config-modal reload-definitions-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Reload workflow definitions result"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="run-config-header">
          <h3 style={{ margin: 0 }}>Reload Workflow Definitions</h3>
          <button type="button" className="secondary" onClick={onClose} disabled={loading}>Close</button>
        </div>

        {loading ? (
          <div className="reload-result-block">
            <p style={{ margin: 0, color: 'var(--muted)' }}>Reloading workflow definitions…</p>
          </div>
        ) : errorMessage ? (
          <div className="reload-result-block reload-result-block--error">
            <p style={{ margin: 0 }}>Reload failed.</p>
            <pre className="reload-result-pre">{errorMessage}</pre>
          </div>
        ) : result ? (
          <>
            <div className="reload-result-block">
              <div className="reload-result-summary">{formatSummary(result)}</div>
              <div className="reload-result-root">
                <span className="reload-result-label">Definitions Root</span>
                <code>{result.definitions_root}</code>
              </div>
            </div>

            {failedRows.length > 0 ? (
              <div className="reload-result-block reload-result-block--error">
                <div className="reload-result-summary">
                  Failed Definitions ({failedRows.length})
                </div>
                <div className="reload-result-list" role="list">
                  {failedRows.map((row) => (
                    <div key={row.file} className="reload-result-item" role="listitem">
                      <code className="reload-result-path">{row.file}</code>
                      <pre className="reload-result-pre">{row.message ?? 'Unknown error'}</pre>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <div className="reload-result-block reload-result-block--ok">
                <p style={{ margin: 0 }}>All discovered definitions were reloaded successfully.</p>
              </div>
            )}

            {skippedRows.length > 0 && (
              <div className="reload-result-block">
                <div className="reload-result-summary">
                  Skipped YAML Files ({skippedRows.length})
                </div>
                <div className="reload-result-list" role="list">
                  {skippedRows.map((row) => (
                    <div key={row.file} className="reload-result-item" role="listitem">
                      <code className="reload-result-path">{row.file}</code>
                      <pre className="reload-result-pre">{row.message ?? 'Skipped.'}</pre>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </>
        ) : null}
      </div>
    </div>
  );
}
