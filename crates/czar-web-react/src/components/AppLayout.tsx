import { useCallback, useEffect, useRef, useState, type MouseEvent as ReactMouseEvent, type ReactNode } from 'react';
import { ChevronLeft, ChevronRight, MoreHorizontal } from 'lucide-react';
import type { Run, Workflow } from '../types';
import type { AppPage, Theme } from '../appState';
import EnvBanner from './EnvBanner';
import PersonalizeModal from './PersonalizeModal';
import LineagePage from './LineagePage';
import StatsPage from './StatsPage';
import SidebarPanels from './SidebarPanels';

interface AppLayoutProps {
  page: AppPage;
  onPageLinkClick: (event: ReactMouseEvent<HTMLAnchorElement>, page: AppPage) => void;
  pageHrefs: Record<AppPage | 'mainWorkflows' | 'mainRuns', string>;
  isMobileLayout: boolean;
  isSidebarOpen: boolean;
  sidebarWidth: number;
  onSidebarWidthChange: (width: number) => void;
  onOpenSidebar: () => void;
  onCloseSidebar: () => void;
  onReloadDefinitions: () => void;
  workflowDefinitionsRoot: string | null;
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
  logDetails: boolean;
  onLogDetailsChange: (value: boolean) => void;
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
  onMainSidebarTabLinkClick: (event: ReactMouseEvent<HTMLAnchorElement>, tab: 'workflows' | 'runs') => void;
  onWorkflowSearchInput: (value: string) => void;
  onWorkflowClick: (workflowName: string) => void;
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
  onRunClick: (runId: string) => void;
  onRunsPageChange: (page: number) => void;
  detailPanel: ReactNode;
}

export default function AppLayout({
  page,
  onPageLinkClick,
  pageHrefs,
  isMobileLayout,
  isSidebarOpen,
  sidebarWidth,
  onSidebarWidthChange,
  onOpenSidebar,
  onCloseSidebar,
  onReloadDefinitions,
  workflowDefinitionsRoot,
  theme,
  onThemeChange,
  logDetails,
  onLogDetailsChange,
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
  onMainSidebarTabLinkClick,
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
  detailPanel,
}: AppLayoutProps) {
  const mainRef = useRef<HTMLElement>(null);
  const isDesktopSidebarHidden = !isMobileLayout && sidebarWidth === 0;
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const mobileMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!mobileMenuOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (mobileMenuRef.current && !mobileMenuRef.current.contains(e.target as Node)) {
        setMobileMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [mobileMenuOpen]);

  const handleDesktopSidebarToggle = useCallback(() => {
    if (isDesktopSidebarHidden) {
      onOpenSidebar();
    } else {
      onSidebarWidthChange(0);
    }
  }, [isDesktopSidebarHidden, onOpenSidebar, onSidebarWidthChange]);

  const handleSidebarSplitterMouseDown = useCallback((e: ReactMouseEvent) => {
    e.preventDefault();
    const main = mainRef.current;
    if (!main) return;

    const startX = e.clientX;
    // Read the sidebar's actual rendered width from the handle's position, not from state,
    // so the drag starts exactly where the user clicked with no jump.
    const startWidth = (e.currentTarget as HTMLElement).getBoundingClientRect().left - main.getBoundingClientRect().left;

    const onMouseMove = (mv: MouseEvent) => {
      onSidebarWidthChange(startWidth + (mv.clientX - startX));
    };

    const onMouseUp = () => {
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    };

    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  }, [onSidebarWidthChange]);

  return (
    <div className={`app${isMobileLayout ? ' app-mobile' : ''}`}>
      <EnvBanner />
      <header className="header">
        <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
          {isMobileLayout && page === 'main' && (
            <button
              className="secondary icon-btn"
              onClick={isSidebarOpen ? onCloseSidebar : onOpenSidebar}
              aria-label={isSidebarOpen ? 'Close sidebar' : 'Open sidebar'}
            >
              {isSidebarOpen ? <ChevronLeft size={16} /> : <ChevronRight size={16} />}
            </button>
          )}
          <h1 style={{ margin: 0 }}>Czar</h1>
          <nav className="header-nav">
            <a
              href={pageHrefs.mainWorkflows}
              data-testid="nav-workflows"
              className={`secondary header-nav-btn${page === 'main' && sidebarTab === 'workflows' ? ' active' : ''}`}
              onClick={(event) => onMainSidebarTabLinkClick(event, 'workflows')}
            >
              Workflows
            </a>
            <a
              href={pageHrefs.mainRuns}
              data-testid="nav-runs"
              className={`secondary header-nav-btn${page === 'main' && sidebarTab === 'runs' ? ' active' : ''}`}
              onClick={(event) => onMainSidebarTabLinkClick(event, 'runs')}
            >
              Runs
            </a>
            {!isMobileLayout && (
              <>
                <a
                  href={pageHrefs.lineage}
                  className={`secondary header-nav-btn${page === 'lineage' ? ' active' : ''}`}
                  onClick={(event) => onPageLinkClick(event, 'lineage')}
                >
                  Lineage
                </a>
                <a
                  href={pageHrefs.stats}
                  className={`secondary header-nav-btn${page === 'stats' ? ' active' : ''}`}
                  onClick={(event) => onPageLinkClick(event, 'stats')}
                >
                  Stats
                </a>
              </>
            )}
          </nav>
        </div>
        <div className="controls">
          {!isMobileLayout && (
            <>
              {workflowDefinitionsRoot && (
                <span
                  style={{ color: 'var(--muted)', fontSize: '0.85rem' }}
                  title={workflowDefinitionsRoot}
                >
                  Definitions root: <code>{workflowDefinitionsRoot}</code>
                </span>
              )}
              <button className="secondary" onClick={onReloadDefinitions}>
                Reload Workflow Definitions
              </button>
            </>
          )}
          <PersonalizeModal theme={theme} onThemeChange={onThemeChange} logDetails={logDetails} onLogDetailsChange={onLogDetailsChange} />
          {isMobileLayout && (
            <div className="mobile-nav-menu" ref={mobileMenuRef}>
              <button
                className="secondary icon-btn"
                onClick={() => setMobileMenuOpen(v => !v)}
                aria-label="More options"
                aria-expanded={mobileMenuOpen}
              >
                <MoreHorizontal size={16} />
              </button>
              {mobileMenuOpen && (
                <div className="mobile-nav-dropdown">
                  <a
                    href={pageHrefs.lineage}
                    className={`mobile-nav-dropdown-item${page === 'lineage' ? ' active' : ''}`}
                    onClick={(e) => { onPageLinkClick(e, 'lineage'); setMobileMenuOpen(false); }}
                  >
                    Lineage
                  </a>
                  <a
                    href={pageHrefs.stats}
                    className={`mobile-nav-dropdown-item${page === 'stats' ? ' active' : ''}`}
                    onClick={(e) => { onPageLinkClick(e, 'stats'); setMobileMenuOpen(false); }}
                  >
                    Stats
                  </a>
                  <button
                    className="mobile-nav-dropdown-item"
                    onClick={() => { onReloadDefinitions(); setMobileMenuOpen(false); }}
                  >
                    Reload Workflow Definitions
                  </button>
                  {workflowDefinitionsRoot && (
                    <div
                      className="mobile-nav-dropdown-item"
                      style={{ color: 'var(--muted)', cursor: 'default' }}
                      title={workflowDefinitionsRoot}
                    >
                      Definitions root: {workflowDefinitionsRoot}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </header>

      {page === 'stats' ? (
        <StatsPage />
      ) : page === 'lineage' ? (
        <LineagePage isMobileLayout={isMobileLayout} />
      ) : (
        <main className="main" ref={mainRef}>
          {isMobileLayout && isSidebarOpen && (
            <button
              type="button"
              className="mobile-sidebar-backdrop"
              aria-label="Close browser panel"
              onClick={onCloseSidebar}
            />
          )}

          {!isMobileLayout && (
            <div className="sidebar-toggle-rail">
              <button
                className="secondary icon-btn"
                onClick={handleDesktopSidebarToggle}
                title={isDesktopSidebarHidden ? 'Open sidebar' : 'Close sidebar'}
              >
                {isDesktopSidebarHidden ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
              </button>
            </div>
          )}

          {!isDesktopSidebarHidden && (
            <aside
              className={`sidebar${isMobileLayout ? ' sidebar-mobile' : ''}${isSidebarOpen ? ' open' : ''}`}
              style={isMobileLayout ? undefined : { width: `${sidebarWidth}px`, flexShrink: 0 }}
            >
              <SidebarPanels
                sidebarTab={sidebarTab}
                workflowsTotal={workflowsTotal}
                runsTotal={runsTotal}
                workflows={workflows}
                selectedWorkflow={selectedWorkflow}
                latestRunByWorkflow={latestRunByWorkflow}
                workflowSearchInput={workflowSearchInput}
                runsSearchInput={runsSearchInput}
                workflowsPage={workflowsPage}
                workflowsTotalPages={workflowsTotalPages}
                onWorkflowSearchInput={onWorkflowSearchInput}
                onWorkflowClick={onWorkflowClick}
                onWorkflowPageChange={onWorkflowPageChange}
                runs={runs}
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
            </aside>
          )}

          {!isMobileLayout && !isDesktopSidebarHidden && (
            <div
              className="split-handle split-handle--vertical"
              onMouseDown={handleSidebarSplitterMouseDown}
            />
          )}

          {detailPanel}
        </main>
      )}
    </div>
  );
}
