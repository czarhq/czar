import { useState } from 'react';
import type { TaskSelectionMode } from '../types';

interface RunConfigurationBodyProps {
  selectionMode: TaskSelectionMode;
  onSelectionModeChange: (mode: TaskSelectionMode) => void;
  flowNames: string[];
  selectedFlowName: string;
  onSelectedFlowNameChange: (flowName: string) => void;
  selectionStateFilter: string;
  selectionStateOptions: string[];
  onSelectionStateFilterChange: (value: string) => void;
  requestedTaskNames: string[];
  previewResolvedTaskNames: string[];
  stateFilteredTaskNames: string[];
  onSelectAll: () => void;
  onClear: () => void;
  onUseStateFilter: () => void;
  onAddStateFilter: () => void;
  onExpandSelection: (mode: Exclude<TaskSelectionMode, 'exact'>) => void;
  onUseResolved: () => void;
  taskGroupEntries: Array<[string, string[]]>;
  selectedGroupNames: string[];
  onToggleTaskGroup: (groupName: string) => void;
  exactModeUnmaterializedDependencyCount: number;
  onTaskFocus: (taskName: string) => void;
}

export default function RunConfigurationBody({
  selectionMode,
  onSelectionModeChange,
  flowNames,
  selectedFlowName,
  onSelectedFlowNameChange,
  selectionStateFilter,
  selectionStateOptions,
  onSelectionStateFilterChange,
  requestedTaskNames,
  previewResolvedTaskNames,
  stateFilteredTaskNames,
  onSelectAll,
  onClear,
  onUseStateFilter,
  onAddStateFilter,
  onExpandSelection,
  onUseResolved,
  taskGroupEntries,
  selectedGroupNames,
  onToggleTaskGroup,
  exactModeUnmaterializedDependencyCount,
  onTaskFocus,
}: RunConfigurationBodyProps) {
  const [toolsOpen, setToolsOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [expandMode, setExpandMode] = useState<Exclude<TaskSelectionMode, 'exact'>>('full');

  return (
    <>
      <div className="execution-toolbar-main">
        <label className="execution-inline-label">
          Mode
          <select
            value={selectionMode}
            onChange={(event) => onSelectionModeChange(event.target.value as TaskSelectionMode)}
          >
            <option value="exact">Selected Only</option>
            <option value="upstream">Selected + Dependencies</option>
            <option value="downstream">Selected + Dependants</option>
            <option value="full">Full Neighborhood</option>
          </select>
        </label>
        {flowNames.length > 0 && (
          <label className="execution-inline-label">
            Flow
            <select
              value={selectedFlowName}
              onChange={(event) => onSelectedFlowNameChange(event.target.value)}
            >
              <option value="">Auto / All Tasks</option>
              {flowNames.map((flowName) => (
                <option key={flowName} value={flowName}>
                  {flowName}
                </option>
              ))}
            </select>
          </label>
        )}
        <label className="execution-inline-label">
          Last State
          <select
            value={selectionStateFilter}
            onChange={(event) => onSelectionStateFilterChange(event.target.value)}
          >
            {selectionStateOptions.map((status) => (
              <option key={status} value={status}>
                {status === 'all' ? 'All' : status === 'never' ? 'Never' : status}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="execution-toolbar-meta run-config-summary">
        <span className="pill">{requestedTaskNames.length} selected</span>
        <span className="pill">{previewResolvedTaskNames.length} final scope</span>
        <span className={`pill execution-state-filter-pill${selectionStateFilter === 'all' ? ' execution-state-filter-pill--hidden' : ''}`}>
          {stateFilteredTaskNames.length} state-filtered
        </span>
        <div className="run-config-summary-actions">
          <button
            type="button"
            className={`secondary run-config-toggle${toolsOpen ? ' active' : ''}`}
            aria-pressed={toolsOpen}
            onClick={() => setToolsOpen((value) => !value)}
          >
            Tools
          </button>
          <button
            type="button"
            className={`secondary run-config-toggle${previewOpen ? ' active' : ''}`}
            aria-pressed={previewOpen}
            onClick={() => setPreviewOpen((value) => !value)}
          >
            Preview
          </button>
        </div>
      </div>

      {toolsOpen && (
        <div className="run-config-tools">
          <div className="run-config-tool-row">
            <span className="run-config-tool-label">Selection</span>
            <button type="button" className="secondary" onClick={onSelectAll}>
              Select All
            </button>
            <button type="button" className="secondary" onClick={onClear}>
              Clear
            </button>
            <button
              type="button"
              className="secondary"
              onClick={onUseResolved}
              disabled={previewResolvedTaskNames.length === 0}
            >
          Apply Final Scope
        </button>
      </div>

          <div className="run-config-tool-row">
            <span className="run-config-tool-label">Expand</span>
            <select
              value={expandMode}
              onChange={(event) => setExpandMode(event.target.value as Exclude<TaskSelectionMode, 'exact'>)}
              className="run-config-expand-select"
            >
              <option value="upstream">Add Dependencies</option>
              <option value="downstream">Add Dependants</option>
              <option value="full">Add Full Neighborhood</option>
            </select>
            <button type="button" className="secondary" onClick={() => onExpandSelection(expandMode)}>
              Apply
            </button>
          </div>

          <div className="run-config-tool-row">
            <span className="run-config-tool-label">State Filter</span>
            <button
              type="button"
              className="secondary"
              onClick={onUseStateFilter}
              disabled={selectionStateFilter === 'all'}
            >
              Replace with Filter
            </button>
            <button
              type="button"
              className="secondary"
              onClick={onAddStateFilter}
              disabled={selectionStateFilter === 'all'}
            >
              Add Filter Matches
            </button>
          </div>

          {taskGroupEntries.length > 0 && (
            <div className="run-config-tool-row run-config-tool-row-groups">
              <span className="run-config-tool-label">Task Groups</span>
              <div className="execution-groups">
                {taskGroupEntries.map(([groupName, tasks]) => {
                  const active = selectedGroupNames.includes(groupName);
                  return (
                    <button
                      type="button"
                      key={groupName}
                      className={active ? 'execution-group-chip active' : 'execution-group-chip'}
                      onClick={() => onToggleTaskGroup(groupName)}
                    >
                      {groupName} ({tasks.length})
                    </button>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      )}

      {exactModeUnmaterializedDependencyCount > 0 && (
        <p className="execution-warning">
          Selected Only: {exactModeUnmaterializedDependencyCount} upstream dependency task(s)
          have no successful prior materialization. Add dependencies with Expand.
        </p>
      )}

      {previewOpen && (
        <div className="selection-preview">
          <div>
            <div className="selection-preview-title">Selected</div>
            <div className="selection-preview-chips">
              {requestedTaskNames.length === 0 ? (
                <span className="selection-preview-empty">none</span>
              ) : (
                requestedTaskNames.map((taskName) => (
                  <button
                    type="button"
                    key={`requested:${taskName}`}
                    className="selection-preview-chip"
                    onClick={() => onTaskFocus(taskName)}
                    title={taskName}
                  >
                    {taskName}
                  </button>
                ))
              )}
            </div>
          </div>
          <div>
            <div className="selection-preview-title">Final Scope</div>
            <div className="selection-preview-chips">
              {previewResolvedTaskNames.length === 0 ? (
                <span className="selection-preview-empty">none</span>
              ) : (
                previewResolvedTaskNames.map((taskName) => (
                  <button
                    type="button"
                    key={`resolved:${taskName}`}
                    className="selection-preview-chip resolved"
                    onClick={() => onTaskFocus(taskName)}
                    title={taskName}
                  >
                    {taskName}
                  </button>
                ))
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
