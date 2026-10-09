import { useState, type ReactNode } from 'react';

interface DetailPanelExecutionToolbarProps {
  isMobileLayout: boolean;
  runTriggering: boolean;
  requestedCount: number;
  resolvedCount: number;
  hasSelection: boolean;
  runDialogOpen: boolean;
  runDialogMode: 'all' | 'selection';
  canSubmitRun: boolean;
  onOpenRunDialog: () => void;
  onCloseRunDialog: () => void;
  onRunDialogModeChange: (mode: 'all' | 'selection') => void;
  onRunAll: () => void;
  onRunSelection: () => void;
  onSubmitRun: () => void;
  renderConfigurationBody: () => ReactNode;
}

export default function DetailPanelExecutionToolbar({
  isMobileLayout,
  runTriggering,
  hasSelection,
  runDialogOpen,
  runDialogMode,
  canSubmitRun,
  onOpenRunDialog,
  onCloseRunDialog,
  onRunDialogModeChange,
  onRunAll,
  onRunSelection,
  onSubmitRun,
  renderConfigurationBody,
}: DetailPanelExecutionToolbarProps) {
  const [configOpen, setConfigOpen] = useState(false);

  return (
    <>
      {isMobileLayout ? (
        <div className="execution-toolbar execution-toolbar--compact">
          <div className="execution-toolbar-main">
            <button
              type="button"
              data-testid="run-workflow-button"
              onClick={onOpenRunDialog}
              disabled={runTriggering}
              className="execution-run-all"
            >
              {runTriggering ? 'Running…' : 'Run'}
            </button>
          </div>
        </div>
      ) : (
        <div className="execution-toolbar">
          <div className="execution-toolbar-main">
            <button
              type="button"
              data-testid="run-workflow-button"
              onClick={onRunAll}
              disabled={runTriggering}
              className="execution-run-all"
            >
              {runTriggering ? 'Running…' : 'Run'}
            </button>
            <button
              type="button"
              className={`secondary run-config-toggle${configOpen ? ' active' : ''}`}
              aria-pressed={configOpen}
              onClick={() => setConfigOpen((v) => !v)}
              title="Configure run"
            >
              ▾
            </button>
          </div>
          {configOpen && (
            <>
              {renderConfigurationBody()}
              <div className="execution-toolbar-main">
                <button
                  type="button"
                  onClick={onRunSelection}
                  disabled={runTriggering || !hasSelection}
                  className="secondary"
                >
                  Run Selection
                </button>
              </div>
            </>
          )}
        </div>
      )}

      {isMobileLayout && runDialogOpen && (
        <div className="modal-backdrop" onClick={onCloseRunDialog}>
          <div
            className="panel run-config-modal"
            role="dialog"
            aria-modal="true"
            aria-label="Run configuration"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="run-config-header">
              <h2>Run Configuration</h2>
              <button type="button" className="secondary" onClick={onCloseRunDialog}>
                Close
              </button>
            </div>

            <div className="run-intent-options">
              <button
                type="button"
                className={runDialogMode === 'all' ? 'run-intent-card active' : 'run-intent-card'}
                onClick={() => onRunDialogModeChange('all')}
              >
                Run All Tasks
              </button>
              <button
                type="button"
                className={runDialogMode === 'selection' ? 'run-intent-card active' : 'run-intent-card'}
                onClick={() => onRunDialogModeChange('selection')}
                disabled={!hasSelection}
              >
                Run Selection
              </button>
            </div>

            {renderConfigurationBody()}

            <div className="run-config-actions">
              <button type="button" className="secondary" onClick={onCloseRunDialog}>
                Cancel
              </button>
              <button
                type="button"
                className="run-confirm-button"
                disabled={!canSubmitRun}
                onClick={onSubmitRun}
              >
                {runDialogMode === 'selection' ? 'Run Selection Now' : 'Run All Now'}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
