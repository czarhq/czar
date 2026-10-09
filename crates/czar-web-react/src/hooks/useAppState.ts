import { useState, useEffect, useCallback, useRef, useMemo, type MouseEvent as ReactMouseEvent } from 'react';
import {
  fetchWorkflowDefinitionsConfig,
  fetchRunDetail,
  reloadWorkflowDefinitions,
  triggerRun,
} from '../api';
import { useRunActions } from './useRunActions';
import { useGraphPresentationData } from './useGraphPresentationData';
import { useMobileSidebar } from './useMobileSidebar';
import { usePollCadence } from './usePollCadence';
import { useRunDetailPolling } from './useRunDetailPolling';
import { useAppData } from './useAppData';
import { useAppHrefs } from './useAppHrefs';
import { useWorkflowRunData } from './useWorkflowRunData';
import { buildAppHref, readSelectionFromUrl } from '../selectionUrl';
import {
  type AppPage,
  DEFAULT_SIDEBAR_WIDTH,
  MAX_SIDEBAR_WIDTH,
  MIN_SIDEBAR_WIDTH,
  MOBILE_LAYOUT_MAX_WIDTH,
  SIDEBAR_COLLAPSE_THRESHOLD,
  WORKFLOW_GRID_MAX_RUNS,
  readDesktopSidebarState,
  readLogDetails,
  readPageFromUrl,
  saveDesktopSidebarState,
  saveLogDetails,
  shouldHandleClientNavigation,
} from '../appState';
import type {
  RunDetail,
  TriggerRunRequestOptions,
  ReloadWorkflowDefinitionsResponse,
} from '../types';

export function useAppState() {
  const [logDetails, setLogDetails] = useState(readLogDetails);
  const handleLogDetailsChange = useCallback((v: boolean) => { saveLogDetails(v); setLogDetails(v); }, []);
  const [page, setPage] = useState<AppPage>(readPageFromUrl);
  const initialSelection = readSelectionFromUrl();
  const initialDesktopSidebarState = useMemo(readDesktopSidebarState, []);
  const [workflowsPage, setWorkflowsPage] = useState(0);
  const [workflowSearchInput, setWorkflowSearchInput] = useState('');
  const [workflowSearch, setWorkflowSearch] = useState('');
  const [selectedWorkflow, setSelectedWorkflow] = useState(initialSelection.workflow);
  const [runsPage, setRunsPage] = useState(0);
  const [runsSearchInput, setRunsSearchInput] = useState('');
  const [runsSearch, setRunsSearch] = useState('');
  const [runsStatusFilter, setRunsStatusFilter] = useState<string[]>([]);
  const [runsWorkflowFilter, setRunsWorkflowFilter] = useState<string[]>([]);
  const [runsDateFilter, setRunsDateFilter] = useState<{ from: string; to: string }>({ from: '', to: '' });
  const [selectedRun, setSelectedRun] = useState(initialSelection.run);
  const [activeTab, setActiveTab] = useState<'run' | 'graph' | 'grid'>(() => {
    if (initialSelection.tab) return initialSelection.tab;
    return initialSelection.run ? 'run' : 'graph';
  });
  const [sidebarTab, setSidebarTab] = useState<'workflows' | 'runs'>(() => (
    initialSelection.sidebar ?? (initialSelection.run ? 'runs' : 'workflows')
  ));
  const [runDetailTab, setRunDetailTab] = useState<'details' | 'gantt'>(() => (
    initialSelection.runDetailTab ?? 'details'
  ));
  const { isMobileLayout, isSidebarOpen, setIsSidebarOpen } = useMobileSidebar(MOBILE_LAYOUT_MAX_WIDTH);

  // When true, the URL sync effect will use pushState instead of replaceState
  const navPushRef = useRef(false);

  const handlePageLinkClick = useCallback((event: ReactMouseEvent<HTMLAnchorElement>, next: AppPage) => {
    if (!shouldHandleClientNavigation(event)) return;
    event.preventDefault();
    navPushRef.current = true;
    setPage(next);
  }, []);

  const handleSidebarTabLinkClick = useCallback((
    event: ReactMouseEvent<HTMLAnchorElement>,
    next: 'workflows' | 'runs',
  ) => {
    if (!shouldHandleClientNavigation(event)) return;
    event.preventDefault();
    navPushRef.current = true;
    setSidebarTab(next);
  }, []);

  const handleMainSidebarTabLinkClick = useCallback((
    event: ReactMouseEvent<HTMLAnchorElement>,
    next: 'workflows' | 'runs',
  ) => {
    if (!shouldHandleClientNavigation(event)) return;
    event.preventDefault();
    navPushRef.current = true;
    setPage('main');
    setSidebarTab(next);
  }, []);

  const handleRunDetailTabLinkClick = useCallback((
    event: ReactMouseEvent<HTMLAnchorElement>,
    next: 'details' | 'gantt',
  ) => {
    if (!shouldHandleClientNavigation(event)) return;
    event.preventDefault();
    navPushRef.current = true;
    setRunDetailTab(next);
  }, []);

  const [sidebarWidth, setSidebarWidth] = useState(initialDesktopSidebarState.width);
  const lastExpandedSidebarWidthRef = useRef(initialDesktopSidebarState.lastExpandedWidth);
  const [graphOrientation, setGraphOrientation] = useState<'LR' | 'TB'>('LR');
  const [runDetail, setRunDetail] = useState<RunDetail | null>(null);
  const runDetailCacheRef = useRef(new Map<string, RunDetail>());
  const [runTriggering, setRunTriggering] = useState(false);
  const [runTriggerError, setRunTriggerError] = useState<string | null>(null);
  const [reloadDefinitionsLoading, setReloadDefinitionsLoading] = useState(false);
  const [reloadDefinitionsResult, setReloadDefinitionsResult] = useState<ReloadWorkflowDefinitionsResponse | null>(null);
  const [reloadDefinitionsError, setReloadDefinitionsError] = useState<string | null>(null);
  const [workflowDefinitionsRoot, setWorkflowDefinitionsRoot] = useState<string | null>(null);
  const [dataRefreshNonce, setDataRefreshNonce] = useState(0);
  const pollCadence = usePollCadence();

  const handleTerminalTransition = useCallback(() => {
    setDataRefreshNonce((n) => n + 1);
  }, []);

  const {
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
  } = useAppData({
    workflowsPage,
    workflowSearch,
    runsPage,
    runsSearch,
    runsWorkflowFilter,
    runsStatusFilter,
    runsDateFilter,
    selectedWorkflow,
    dataRefreshNonce,
  });

  useEffect(() => {
    saveDesktopSidebarState({
      width: sidebarWidth,
      lastExpandedWidth: lastExpandedSidebarWidthRef.current,
    });
  }, [sidebarWidth]);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      setWorkflowsPage(0);
      setWorkflowSearch(workflowSearchInput);
    }, 300);
    return () => window.clearTimeout(timeout);
  }, [workflowSearchInput]);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      setRunsPage(0);
      setRunsSearch(runsSearchInput);
    }, 300);
    return () => window.clearTimeout(timeout);
  }, [runsSearchInput]);

  const {
    workflowHistoryRuns,
    workflowHistoryLoading,
    workflowLastRunByName,
  } = useWorkflowRunData({
    workflows,
    selectedWorkflow,
    dataRefreshNonce,
    workflowGridMaxRuns: WORKFLOW_GRID_MAX_RUNS,
  });

  useEffect(() => {
    if (typeof window === 'undefined') return;
    const next = buildAppHref({
      workflow: selectedWorkflow,
      run: selectedRun,
      tab: activeTab,
      sidebar: sidebarTab,
      runDetailTab,
      page,
    });
    const current = `${window.location.pathname}${window.location.search}${window.location.hash}`;
    if (next !== current) {
      if (navPushRef.current) {
        window.history.pushState(null, '', next);
      } else {
        window.history.replaceState(null, '', next);
      }
    }
    navPushRef.current = false;
  }, [activeTab, page, runDetailTab, selectedRun, selectedWorkflow, sidebarTab]);

  useEffect(() => {
    const handlePopState = () => {
      const sel = readSelectionFromUrl();
      const pg = readPageFromUrl();
      setPage(pg);
      setSelectedWorkflow(sel.workflow);
      setSelectedRun(sel.run);
      setActiveTab(sel.tab ?? (sel.run ? 'run' : 'graph'));
      setSidebarTab(sel.sidebar ?? (sel.run ? 'runs' : 'workflows'));
      setRunDetailTab(sel.runDetailTab ?? 'details');
      setRunDetail(null);
    };
    window.addEventListener('popstate', handlePopState);
    return () => window.removeEventListener('popstate', handlePopState);
  }, []);

  useRunDetailPolling({
    selectedRun,
    pollCadence,
    runDetail,
    setRunDetail,
    onTerminalTransition: handleTerminalTransition,
  });

  const selectedRunRef = useRef(selectedRun);
  selectedRunRef.current = selectedRun;

  useEffect(() => {
    if (!runDetail) return;
    runDetailCacheRef.current.set(runDetail.run.id, runDetail);
  }, [runDetail]);

  const displayRunDetail = useMemo(() => {
    if (!selectedRun) return runDetail;
    if (runDetail?.run.id === selectedRun) return runDetail;
    return runDetailCacheRef.current.get(selectedRun) ?? runDetail;
  }, [runDetail, selectedRun]);

  const handleSidebarWidthChange = useCallback((width: number) => {
    const clamped = Math.min(MAX_SIDEBAR_WIDTH, Math.max(0, width));
    const nextWidth = clamped <= SIDEBAR_COLLAPSE_THRESHOLD
      ? 0
      : Math.max(MIN_SIDEBAR_WIDTH, clamped);
    if (nextWidth > 0) {
      lastExpandedSidebarWidthRef.current = nextWidth;
    }
    setSidebarWidth(nextWidth);
  }, []);

  const handleOpenSidebar = useCallback(() => {
    if (isMobileLayout) {
      setIsSidebarOpen(true);
      return;
    }
    setSidebarWidth(lastExpandedSidebarWidthRef.current);
  }, [isMobileLayout, setIsSidebarOpen]);

  const handleCloseSidebar = useCallback(() => {
    if (isMobileLayout) {
      setIsSidebarOpen(false);
      return;
    }
    setSidebarWidth(0);
  }, [isMobileLayout, setIsSidebarOpen]);

  const setRunDetailIfCurrent = useCallback((expectedRunId: string, detail: RunDetail) => {
    if (selectedRunRef.current === expectedRunId) {
      setRunDetail(detail);
    }
  }, []);

  useEffect(() => {
    fetchWorkflowDefinitionsConfig()
      .then((config) => setWorkflowDefinitionsRoot(config.definitions_root))
      .catch(console.error);
  }, []);

  useEffect(() => {
    if (!selectedWorkflow && !selectedRun && workflows.length > 0) {
      setSelectedWorkflow(workflows[0].name);
    }
  }, [workflows, selectedWorkflow, selectedRun]);

  const refreshAll = useCallback(() => {
    setDataRefreshNonce((n) => n + 1);
    if (selectedRunRef.current) {
      const runId = selectedRunRef.current;
      fetchRunDetail(runId)
        .then((detail) => setRunDetailIfCurrent(runId, detail))
        .catch(console.error);
    }
  }, [setRunDetailIfCurrent]);

  const handleReloadDefinitions = useCallback(async () => {
    setReloadDefinitionsError(null);
    setReloadDefinitionsResult(null);
    setReloadDefinitionsLoading(true);
    try {
      const result = await reloadWorkflowDefinitions();
      setReloadDefinitionsResult(result);
      setWorkflowDefinitionsRoot(result.definitions_root);
      refreshAll();
    } catch (error) {
      console.error(error);
      setReloadDefinitionsError(error instanceof Error ? error.message : 'Failed to reload workflow definitions.');
    } finally {
      setReloadDefinitionsLoading(false);
    }
  }, [refreshAll]);

  const handleCloseReloadDefinitionsModal = useCallback(() => {
    setReloadDefinitionsLoading(false);
    setReloadDefinitionsResult(null);
    setReloadDefinitionsError(null);
  }, []);

  const {
    cancelRunIdToConfirm,
    cancelRunSubmitting,
    cancelRunError,
    runActionError,
    setRunActionError,
    handleCancelRun,
    handleCloseCancelRunModal,
    handleConfirmCancelRun,
    handlePauseRun,
    handleResumeRun,
  } = useRunActions(refreshAll);

  const handleWorkflowClick = useCallback((name: string) => {
    navPushRef.current = true;
    setSelectedWorkflow(name);
    setSelectedRun('');
    setRunDetail(null);
    setSidebarTab('workflows');
    setActiveTab('graph');
    if (isMobileLayout) {
      setIsSidebarOpen(false);
    }
  }, [isMobileLayout, setIsSidebarOpen]);

  const handleRunClick = useCallback((id: string) => {
    if (id === selectedRunRef.current) {
      navPushRef.current = true;
      setActiveTab('run');
      return;
    }
    navPushRef.current = true;
    const clickedRun = runs.find((run) => run.id === id);
    if (clickedRun?.workflow) {
      setSelectedWorkflow(clickedRun.workflow);
    }
    setSelectedRun(id);
    setSidebarTab('runs');
    setActiveTab('run');
    setRunDetailTab('details');
    if (isMobileLayout) {
      setIsSidebarOpen(false);
    }
  }, [isMobileLayout, runs, setIsSidebarOpen]);

  const handleGridRunClick = useCallback((id: string) => {
    if (id === selectedRunRef.current) return;
    navPushRef.current = true;
    setSelectedRun(id);
    setRunDetailTab('details');
  }, []);

  const handleRunSelectedWorkflow = useCallback(async (options?: TriggerRunRequestOptions) => {
    if (!selectedWorkflow || runTriggering) return;
    setRunTriggerError(null);
    setRunTriggering(true);
    try {
      const result = await triggerRun(selectedWorkflow, options);
      if (result.run_id) {
        navPushRef.current = true;
        setRunsPage(0);
        setSelectedRun(result.run_id);
        setSidebarTab('runs');
        setActiveTab('run');
        setRunDetailTab('details');
        fetchRunDetail(result.run_id)
          .then((detail) => setRunDetailIfCurrent(result.run_id, detail))
          .catch(console.error);
        setDataRefreshNonce((n) => n + 1);
      }
    } catch (error) {
      console.error(error);
      setRunTriggerError(
        error instanceof Error
          ? `Failed to trigger workflow "${selectedWorkflow}": ${error.message}`
          : `Failed to trigger workflow "${selectedWorkflow}".`
      );
    } finally {
      setRunTriggering(false);
    }
  }, [selectedWorkflow, runTriggering, setRunDetailIfCurrent]);

  const { pageHrefs, sidebarHrefs, runDetailTabHrefs } = useAppHrefs({
    page,
    activeTab,
    sidebarTab,
    runDetailTab,
    selectedWorkflow,
    selectedRun,
  });

  const {
    runGraphNodes,
    runStatusMap,
    runProgressMap,
    workflowGraphNodes,
    workflowStatusMap,
    workflowGridTaskNames,
  } = useGraphPresentationData({
    runDetail: displayRunDetail,
    workflowDetail,
    workflowTaskStates,
    workflowHistoryRuns,
  });

  return {
    // theme is handled in App directly via useTheme
    logDetails,
    handleLogDetailsChange,
    page,
    workflowsPage,
    setWorkflowsPage,
    workflowSearchInput,
    setWorkflowSearchInput,
    selectedWorkflow,
    runsPage,
    setRunsPage,
    runsSearchInput,
    setRunsSearchInput,
    runsStatusFilter,
    setRunsStatusFilter,
    runsWorkflowFilter,
    setRunsWorkflowFilter,
    runsDateFilter,
    setRunsDateFilter,
    selectedRun,
    activeTab,
    sidebarTab,
    runDetailTab,
    isMobileLayout,
    isSidebarOpen,
    sidebarWidth,
    graphOrientation,
    setGraphOrientation,
    runDetail: displayRunDetail,
    runTriggering,
    runTriggerError,
    setRunTriggerError,
    cancelRunIdToConfirm,
    cancelRunSubmitting,
    cancelRunError,
    runActionError,
    setRunActionError,
    reloadDefinitionsLoading,
    reloadDefinitionsResult,
    reloadDefinitionsError,
    workflowDefinitionsRoot,
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
    workflowHistoryRuns,
    workflowHistoryLoading,
    workflowLastRunByName,
    runGraphNodes,
    runStatusMap,
    runProgressMap,
    workflowGraphNodes,
    workflowStatusMap,
    workflowGridTaskNames,
    pageHrefs,
    sidebarHrefs,
    runDetailTabHrefs,
    handlePageLinkClick,
    handleSidebarTabLinkClick,
    handleMainSidebarTabLinkClick,
    handleRunDetailTabLinkClick,
    handleSidebarWidthChange,
    handleOpenSidebar,
    handleCloseSidebar,
    handleReloadDefinitions,
    handleCloseReloadDefinitionsModal,
    handleCancelRun,
    handleCloseCancelRunModal,
    handleConfirmCancelRun,
    handlePauseRun,
    handleResumeRun,
    handleWorkflowClick,
    handleRunClick,
    handleGridRunClick,
    handleRunSelectedWorkflow,
  };
}

// Re-export constants used by App for sidebar width
export { DEFAULT_SIDEBAR_WIDTH };
