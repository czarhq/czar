export type UiTab = 'run' | 'graph' | 'grid';
export type SidebarTab = 'workflows' | 'runs';
export type AppPage = 'main' | 'lineage' | 'stats';
export type RunDetailTab = 'details' | 'gantt';

function parseTab(value: string | null): UiTab | null {
  if (value === 'run' || value === 'graph' || value === 'grid') {
    return value;
  }
  return null;
}

function parseSidebarTab(value: string | null): SidebarTab | null {
  if (value === 'workflows' || value === 'runs') {
    return value;
  }
  return null;
}

function parseRunDetailTab(value: string | null): RunDetailTab | null {
  if (value === 'details' || value === 'gantt') {
    return value;
  }
  return null;
}

function setOptionalParam(params: URLSearchParams, key: string, value: string | null | undefined) {
  const trimmed = value?.trim();
  if (trimmed) {
    params.set(key, trimmed);
    return;
  }
  params.delete(key);
}

export function readSelectionFromUrl(): {
  workflow: string;
  run: string;
  tab: UiTab | null;
  sidebar: SidebarTab | null;
  runDetailTab: RunDetailTab | null;
} {
  if (typeof window === 'undefined') {
    return { workflow: '', run: '', tab: null, sidebar: null, runDetailTab: null };
  }
  const params = new URLSearchParams(window.location.search);
  return {
    workflow: params.get('workflow')?.trim() ?? '',
    run: params.get('run')?.trim() ?? '',
    tab: parseTab(params.get('tab')),
    sidebar: parseSidebarTab(params.get('sidebar')),
    runDetailTab: parseRunDetailTab(params.get('run_detail_tab')),
  };
}

export function buildAppHref(
  {
    workflow,
    run,
    tab,
    sidebar,
    runDetailTab,
    page,
  }: {
    workflow?: string;
    run?: string;
    tab?: UiTab | null;
    sidebar?: SidebarTab | null;
    runDetailTab?: RunDetailTab | null;
    page?: AppPage;
  },
  currentHref?: string,
): string {
  const baseHref = currentHref
    ?? (typeof window === 'undefined' ? 'http://localhost/' : window.location.href);
  const url = new URL(baseHref);

  if (workflow !== undefined) {
    setOptionalParam(url.searchParams, 'workflow', workflow);
  }
  if (run !== undefined) {
    setOptionalParam(url.searchParams, 'run', run);
  }
  if (tab !== undefined) {
    setOptionalParam(url.searchParams, 'tab', tab);
  }
  if (sidebar !== undefined) {
    setOptionalParam(url.searchParams, 'sidebar', sidebar);
  }
  if (runDetailTab !== undefined) {
    setOptionalParam(url.searchParams, 'run_detail_tab', runDetailTab);
  }
  if (page !== undefined) {
    if (page === 'main') {
      url.searchParams.delete('page');
    } else {
      url.searchParams.set('page', page);
    }
  }

  return `${url.pathname}${url.search}${url.hash}`;
}
