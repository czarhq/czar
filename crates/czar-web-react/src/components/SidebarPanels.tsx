import type { Run, Workflow } from '../types';
import RunsPanel from './RunsPanel';
import WorkflowPanel from './WorkflowPanel';

interface SidebarPanelsProps {
  sidebarTab: 'workflows' | 'runs';
  workflowsTotal: number;
  runsTotal: number;
  workflows: Workflow[];
  selectedWorkflow: string;
  latestRunByWorkflow: Record<string, Run | null>;
  workflowSearchInput: string;
  runsSearchInput: string;
  workflowsPage: number;
  workflowsTotalPages: number;
  onWorkflowSearchInput: (value: string) => void;
  onWorkflowClick: (name: string) => void;
  onWorkflowPageChange: (page: number) => void;
  runs: Run[];
  selectedRun: string;
  runsStatusFilter: string[];
  runsWorkflowFilter: string[];
  runsDateFilter: { from: string; to: string };
  workflowSuggestions: string[];
  runsPage: number;
  runsTotalPages: number;
  onRunsSearchInput: (value: string) => void;
  onRunsStatusFilterChange: (status: string[]) => void;
  onRunsWorkflowFilterChange: (workflowName: string[]) => void;
  onRunsDateFilterChange: (range: { from: string; to: string }) => void;
  onRunClick: (id: string) => void;
  onRunsPageChange: (page: number) => void;
}

export default function SidebarPanels({
  sidebarTab,
  workflowsTotal,
  runsTotal,
  workflows,
  selectedWorkflow,
  latestRunByWorkflow,
  workflowSearchInput,
  runsSearchInput,
  workflowsPage,
  workflowsTotalPages,
  onWorkflowSearchInput,
  onWorkflowClick,
  onWorkflowPageChange,
  runs,
  selectedRun,
  runsStatusFilter,
  runsWorkflowFilter,
  runsDateFilter,
  workflowSuggestions,
  runsPage,
  runsTotalPages,
  onRunsSearchInput,
  onRunsStatusFilterChange,
  onRunsWorkflowFilterChange,
  onRunsDateFilterChange,
  onRunClick,
  onRunsPageChange,
}: SidebarPanelsProps) {
  return (
    <>
      {sidebarTab === 'workflows' ? (
        <WorkflowPanel
          workflows={workflows}
          selectedWorkflow={selectedWorkflow}
          latestRunByWorkflow={latestRunByWorkflow}
          workflowSearchInput={workflowSearchInput}
          workflowsTotal={workflowsTotal}
          workflowsPage={workflowsPage}
          workflowsTotalPages={workflowsTotalPages}
          onWorkflowSearchInput={onWorkflowSearchInput}
          onWorkflowClick={onWorkflowClick}
          onWorkflowPageChange={onWorkflowPageChange}
        />
      ) : (
        <RunsPanel
          runs={runs}
          runsTotal={runsTotal}
          runsSearchInput={runsSearchInput}
          selectedRun={selectedRun}
          runsStatusFilter={runsStatusFilter}
          runsWorkflowFilter={runsWorkflowFilter}
          runsDateFilter={runsDateFilter}
          workflowSuggestions={workflowSuggestions}
          runsPage={runsPage}
          runsTotalPages={runsTotalPages}
          onRunsSearchInput={onRunsSearchInput}
          onRunsStatusFilterChange={onRunsStatusFilterChange}
          onRunsWorkflowFilterChange={onRunsWorkflowFilterChange}
          onRunsDateFilterChange={onRunsDateFilterChange}
          onRunClick={onRunClick}
          onRunsPageChange={onRunsPageChange}
        />
      )}
    </>
  );
}
