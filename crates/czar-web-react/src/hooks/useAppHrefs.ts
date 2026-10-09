import { useMemo } from 'react';
import { buildAppHref } from '../selectionUrl';
import type { AppPage } from '../appState';

interface UseAppHrefsArgs {
  page: AppPage;
  activeTab: 'run' | 'graph' | 'grid';
  sidebarTab: 'workflows' | 'runs';
  runDetailTab: 'details' | 'gantt';
  selectedWorkflow: string;
  selectedRun: string;
}

export function useAppHrefs({
  page,
  activeTab,
  sidebarTab,
  runDetailTab,
  selectedWorkflow,
  selectedRun,
}: UseAppHrefsArgs) {
  const pageHrefs = useMemo(() => ({
    main: buildAppHref({
      workflow: selectedWorkflow,
      run: selectedRun,
      tab: activeTab,
      sidebar: sidebarTab,
      runDetailTab,
      page: 'main',
    }),
    mainWorkflows: buildAppHref({
      workflow: selectedWorkflow,
      run: selectedRun,
      tab: activeTab,
      sidebar: 'workflows',
      runDetailTab,
      page: 'main',
    }),
    mainRuns: buildAppHref({
      workflow: selectedWorkflow,
      run: selectedRun,
      tab: activeTab,
      sidebar: 'runs',
      runDetailTab,
      page: 'main',
    }),
    lineage: buildAppHref({
      workflow: selectedWorkflow,
      run: selectedRun,
      tab: activeTab,
      sidebar: sidebarTab,
      runDetailTab,
      page: 'lineage',
    }),
    stats: buildAppHref({
      workflow: selectedWorkflow,
      run: selectedRun,
      tab: activeTab,
      sidebar: sidebarTab,
      runDetailTab,
      page: 'stats',
    }),
  }), [activeTab, runDetailTab, selectedRun, selectedWorkflow, sidebarTab]);

  const sidebarHrefs = useMemo(() => ({
    workflows: buildAppHref({
      workflow: selectedWorkflow,
      run: selectedRun,
      tab: activeTab,
      sidebar: 'workflows',
      runDetailTab,
      page,
    }),
    runs: buildAppHref({
      workflow: selectedWorkflow,
      run: selectedRun,
      tab: activeTab,
      sidebar: 'runs',
      runDetailTab,
      page,
    }),
  }), [activeTab, page, runDetailTab, selectedRun, selectedWorkflow]);

  const runDetailTabHrefs = useMemo(() => ({
    details: buildAppHref({
      workflow: selectedWorkflow,
      run: selectedRun,
      tab: activeTab,
      sidebar: sidebarTab,
      runDetailTab: 'details',
      page,
    }),
    gantt: buildAppHref({
      workflow: selectedWorkflow,
      run: selectedRun,
      tab: activeTab,
      sidebar: sidebarTab,
      runDetailTab: 'gantt',
      page,
    }),
  }), [activeTab, page, selectedRun, selectedWorkflow, sidebarTab]);

  return { pageHrefs, sidebarHrefs, runDetailTabHrefs };
}
