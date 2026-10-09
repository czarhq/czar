import { useEffect, useRef, useState } from 'react';
import { fetchRunDetail, fetchRuns } from '../api';
import type { Run, RunDetail, Workflow } from '../types';

type UseWorkflowRunDataArgs = {
  workflows: Workflow[];
  selectedWorkflow: string;
  dataRefreshNonce: number;
  workflowGridMaxRuns: number;
};

export function useWorkflowRunData({
  workflows,
  selectedWorkflow,
  dataRefreshNonce,
  workflowGridMaxRuns,
}: UseWorkflowRunDataArgs) {
  const [workflowHistoryRuns, setWorkflowHistoryRuns] = useState<RunDetail[]>([]);
  const [workflowHistoryLoading, setWorkflowHistoryLoading] = useState(false);
  const [workflowLastRunByName, setWorkflowLastRunByName] = useState<Record<string, Run | null>>(
    {}
  );

  const workflowLastRunsRequestIdRef = useRef(0);
  useEffect(() => {
    if (workflows.length === 0) {
      setWorkflowLastRunByName({});
      return;
    }

    const requestId = ++workflowLastRunsRequestIdRef.current;
    const names = workflows.map((workflow) => workflow.name);

    Promise.all(
      names.map(async (name) => {
        try {
          const response = await fetchRuns(1, 0, '', [name], []);
          return [name, response.items[0] ?? null] as const;
        } catch (error) {
          console.error(error);
          return [name, null] as const;
        }
      })
    ).then((entries) => {
      if (requestId !== workflowLastRunsRequestIdRef.current) return;
      const next: Record<string, Run | null> = {};
      entries.forEach(([name, run]) => {
        next[name] = run;
      });
      setWorkflowLastRunByName(next);
    });
  }, [workflows, dataRefreshNonce]);

  const workflowHistoryRequestIdRef = useRef(0);
  useEffect(() => {
    if (!selectedWorkflow) {
      setWorkflowHistoryRuns([]);
      setWorkflowHistoryLoading(false);
      return;
    }

    const requestId = ++workflowHistoryRequestIdRef.current;
    setWorkflowHistoryLoading(true);

    fetchRuns(1, 0, '', [selectedWorkflow], [])
      .then(async (summary) => {
        const targetRuns = Math.min(summary.total, workflowGridMaxRuns);
        if (targetRuns === 0) return [];

        const data = await fetchRuns(targetRuns, 0, '', [selectedWorkflow], []);
        const details = await Promise.all(
          data.items.map(async (run) => {
            try {
              return await fetchRunDetail(run.id, true);
            } catch (error) {
              console.error(error);
              return null;
            }
          })
        );
        return details.filter((item): item is RunDetail => item !== null);
      })
      .then((details) => {
        if (requestId !== workflowHistoryRequestIdRef.current) return;
        setWorkflowHistoryRuns(details);
      })
      .catch((error) => {
        if (requestId !== workflowHistoryRequestIdRef.current) return;
        console.error(error);
        setWorkflowHistoryRuns([]);
      })
      .finally(() => {
        if (requestId !== workflowHistoryRequestIdRef.current) return;
        setWorkflowHistoryLoading(false);
      });
  }, [selectedWorkflow, dataRefreshNonce, workflowGridMaxRuns]);

  return {
    workflowHistoryRuns,
    workflowHistoryLoading,
    workflowLastRunByName,
  };
}
