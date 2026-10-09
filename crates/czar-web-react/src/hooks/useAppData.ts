import { useEffect, useMemo, useRef, useState } from 'react';
import {
  fetchRuns,
  fetchWorkflowDetail,
  fetchWorkflowTaskStates,
  fetchWorkflows,
} from '../api';
import type {
  Run,
  Workflow,
  WorkflowInfo,
  WorkflowTaskStateListItem,
} from '../types';
import { RUNS_PER_PAGE, WORKFLOWS_PER_PAGE } from '../appState';

interface UseAppDataArgs {
  workflowsPage: number;
  workflowSearch: string;
  runsPage: number;
  runsSearch: string;
  runsWorkflowFilter: string[];
  runsStatusFilter: string[];
  runsDateFilter: { from: string; to: string };
  selectedWorkflow: string;
  dataRefreshNonce: number;
}

export function useAppData({
  workflowsPage,
  workflowSearch,
  runsPage,
  runsSearch,
  runsWorkflowFilter,
  runsStatusFilter,
  runsDateFilter,
  selectedWorkflow,
  dataRefreshNonce,
}: UseAppDataArgs) {
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [workflowsTotal, setWorkflowsTotal] = useState(0);
  const [runs, setRuns] = useState<Run[]>([]);
  const [runsTotal, setRunsTotal] = useState(0);
  const [workflowDetail, setWorkflowDetail] = useState<WorkflowInfo | null>(null);
  const [workflowTaskStates, setWorkflowTaskStates] = useState<WorkflowTaskStateListItem[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    fetchWorkflows(WORKFLOWS_PER_PAGE, workflowsPage * WORKFLOWS_PER_PAGE, workflowSearch)
      .then((data) => {
        setLoadError(null);
        setWorkflows(data.items);
        setWorkflowsTotal(data.total);
      })
      .catch((error) => {
        console.error(error);
        setWorkflows([]);
        setWorkflowsTotal(0);
        setLoadError(error instanceof Error ? error.message : 'Failed to load workflows.');
      });
  }, [workflowsPage, workflowSearch, dataRefreshNonce]);

  const runsRequestIdRef = useRef(0);
  useEffect(() => {
    const requestId = ++runsRequestIdRef.current;
    fetchRuns(RUNS_PER_PAGE, runsPage * RUNS_PER_PAGE, runsSearch, runsWorkflowFilter, runsStatusFilter, runsDateFilter)
      .then((data) => {
        if (requestId !== runsRequestIdRef.current) return;
        setLoadError(null);
        setRuns(data.items);
        setRunsTotal(data.total);
      })
      .catch((error) => {
        if (requestId !== runsRequestIdRef.current) return;
        console.error(error);
        setRuns([]);
        setRunsTotal(0);
        setLoadError(error instanceof Error ? error.message : 'Failed to load runs.');
      });
  }, [runsPage, runsSearch, runsWorkflowFilter, runsStatusFilter, runsDateFilter, dataRefreshNonce]);

  const workflowDetailRequestIdRef = useRef(0);
  useEffect(() => {
    if (!selectedWorkflow) {
      setWorkflowDetail(null);
      return;
    }
    const requestId = ++workflowDetailRequestIdRef.current;
    fetchWorkflowDetail(selectedWorkflow)
      .then((detail) => {
        if (requestId !== workflowDetailRequestIdRef.current) return;
        setWorkflowDetail(detail);
      })
      .catch((error) => {
        if (requestId !== workflowDetailRequestIdRef.current) return;
        console.error(error);
      });
  }, [selectedWorkflow, dataRefreshNonce]);

  const workflowTaskStatesRequestIdRef = useRef(0);
  useEffect(() => {
    if (!selectedWorkflow) {
      setWorkflowTaskStates([]);
      return;
    }
    const requestId = ++workflowTaskStatesRequestIdRef.current;
    fetchWorkflowTaskStates(selectedWorkflow)
      .then((response) => {
        if (requestId !== workflowTaskStatesRequestIdRef.current) return;
        setWorkflowTaskStates(response.items ?? []);
      })
      .catch((error) => {
        if (requestId !== workflowTaskStatesRequestIdRef.current) return;
        console.error(error);
        setWorkflowTaskStates([]);
      });
  }, [selectedWorkflow, dataRefreshNonce]);

  const workflowSuggestions = useMemo(
    () => [...new Set(
      [
        ...workflows.map((w) => w.name),
        ...runs.map((r) => r.workflow),
        selectedWorkflow,
        ...runsWorkflowFilter,
      ]
        .map((name) => name.trim())
        .filter(Boolean)
    )].sort(),
    [runs, runsWorkflowFilter, selectedWorkflow, workflows]
  );

  const workflowsTotalPages = Math.ceil(workflowsTotal / WORKFLOWS_PER_PAGE);
  const runsTotalPages = Math.ceil(runsTotal / RUNS_PER_PAGE);

  return {
    workflows,
    workflowsTotal,
    runs,
    runsTotal,
    workflowDetail,
    workflowTaskStates,
    loadError,
    setLoadError,
    workflowSuggestions,
    workflowsTotalPages,
    runsTotalPages,
  };
}
