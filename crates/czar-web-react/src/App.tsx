import { useFuzzySearch } from './hooks/useFuzzySearch';
import { useTheme } from './hooks/useTheme';
import { useAppState } from './hooks/useAppState';
import AppLayout from './components/AppLayout';
import DetailPanel from './components/DetailPanel';
import FuzzySearchModal from './components/FuzzySearchModal';
import MessageModal from './components/MessageModal';
import ReloadDefinitionsModal from './components/ReloadDefinitionsModal';
import './style.css';

export default function App() {
  const { theme, effectiveTheme, setTheme } = useTheme();
  const s = useAppState();

  const { isOpen: isFuzzySearchOpen, handleSelectRun: handleFuzzySearchSelectRun, handleSelectWorkflow: handleFuzzySearchSelectWorkflow, close: closeFuzzySearch } = useFuzzySearch(s.handleRunClick, s.handleWorkflowClick);

  const detailPanel = (
    <DetailPanel
      isMobileLayout={s.isMobileLayout}
      selectedWorkflow={s.selectedWorkflow}
      selectedRun={s.selectedRun}
      graphOrientation={s.graphOrientation}
      runDetail={s.runDetail}
      runDetailTab={s.runDetailTab}
      workflowDetail={s.workflowDetail}
      workflowTaskStates={s.workflowTaskStates}
      workflowHistoryRuns={s.workflowHistoryRuns}
      workflowHistoryLoading={s.workflowHistoryLoading}
      workflowGridTaskNames={s.workflowGridTaskNames}
      runGraphNodes={s.runGraphNodes}
      runStatusMap={s.runStatusMap}
      runProgressMap={s.runProgressMap}
      workflowGraphNodes={s.workflowGraphNodes}
      workflowStatusMap={s.workflowStatusMap}
      onGraphOrientationChange={s.setGraphOrientation}
      runTriggering={s.runTriggering}
      onGridRunSelect={s.handleGridRunClick}
      onRunWorkflow={s.handleRunSelectedWorkflow}
      onPauseRun={s.handlePauseRun}
      onResumeRun={s.handleResumeRun}
      onCancelRun={s.handleCancelRun}
      onRunDetailTabLinkClick={s.handleRunDetailTabLinkClick}
      runDetailTabHrefs={s.runDetailTabHrefs}
      colorMode={effectiveTheme}
      logDetails={s.logDetails}
    />
  );

  return (
    <>
      <AppLayout
        page={s.page}
        onPageLinkClick={s.handlePageLinkClick}
        pageHrefs={s.pageHrefs}
        isMobileLayout={s.isMobileLayout}
        isSidebarOpen={s.isSidebarOpen}
        sidebarWidth={s.sidebarWidth}
        onSidebarWidthChange={s.handleSidebarWidthChange}
        onOpenSidebar={s.handleOpenSidebar}
        onCloseSidebar={s.handleCloseSidebar}
        onReloadDefinitions={s.handleReloadDefinitions}
        workflowDefinitionsRoot={s.workflowDefinitionsRoot}
        theme={theme}
        onThemeChange={setTheme}
        logDetails={s.logDetails}
        onLogDetailsChange={s.handleLogDetailsChange}
        sidebarTab={s.sidebarTab}
        workflowsTotal={s.workflowsTotal}
        runsTotal={s.runsTotal}
        workflows={s.workflows}
        selectedWorkflow={s.selectedWorkflow}
        latestRunByWorkflow={s.workflowLastRunByName}
        workflowSearchInput={s.workflowSearchInput}
        runsSearchInput={s.runsSearchInput}
        workflowsPage={s.workflowsPage}
        workflowsTotalPages={s.workflowsTotalPages}
        onMainSidebarTabLinkClick={s.handleMainSidebarTabLinkClick}
        onWorkflowSearchInput={s.setWorkflowSearchInput}
        onWorkflowClick={s.handleWorkflowClick}
        onWorkflowPageChange={s.setWorkflowsPage}
        runs={s.runs}
        selectedRun={s.selectedRun}
        runsStatusFilter={s.runsStatusFilter}
        runsWorkflowFilter={s.runsWorkflowFilter}
        runsDateFilter={s.runsDateFilter}
        workflowSuggestions={s.workflowSuggestions}
        runsPage={s.runsPage}
        runsTotalPages={s.runsTotalPages}
        onRunsSearchInput={s.setRunsSearchInput}
        onRunsStatusFilterChange={(statuses) => {
          s.setRunsStatusFilter(statuses);
          s.setRunsPage(0);
        }}
        onRunsWorkflowFilterChange={(workflowNames) => {
          s.setRunsWorkflowFilter(workflowNames);
          s.setRunsPage(0);
        }}
        onRunsDateFilterChange={(range) => {
          s.setRunsDateFilter(range);
          s.setRunsPage(0);
        }}
        onRunClick={s.handleRunClick}
        onRunsPageChange={s.setRunsPage}
        detailPanel={detailPanel}
      />

      <ReloadDefinitionsModal
        loading={s.reloadDefinitionsLoading}
        result={s.reloadDefinitionsResult}
        errorMessage={s.reloadDefinitionsError}
        onClose={s.handleCloseReloadDefinitionsModal}
      />

      <MessageModal
        open={Boolean(s.runTriggerError)}
        title="Run Trigger Failed"
        message={s.runTriggerError ?? ''}
        actions={[{ label: 'Close', onClick: () => s.setRunTriggerError(null), kind: 'secondary' }]}
        onClose={() => s.setRunTriggerError(null)}
      />

      <MessageModal
        open={Boolean(s.loadError)}
        title="Load Failed"
        message={s.loadError ?? ''}
        actions={[{ label: 'Close', onClick: () => s.setLoadError(null), kind: 'secondary' }]}
        onClose={() => s.setLoadError(null)}
      />

      <MessageModal
        open={Boolean(s.runActionError)}
        title="Run Action Failed"
        message={s.runActionError ?? ''}
        actions={[{ label: 'Close', onClick: () => s.setRunActionError(null), kind: 'secondary' }]}
        onClose={() => s.setRunActionError(null)}
      />

      <MessageModal
        open={Boolean(s.cancelRunIdToConfirm)}
        title="Cancel Run"
        message={`Cancel run ${s.cancelRunIdToConfirm ?? ''}?`}
        details={s.cancelRunError ?? undefined}
        actions={[
          { label: 'Keep Running', onClick: s.handleCloseCancelRunModal, kind: 'secondary', disabled: s.cancelRunSubmitting },
          { label: s.cancelRunSubmitting ? 'Cancelling…' : 'Cancel Run', onClick: s.handleConfirmCancelRun, kind: 'danger', disabled: s.cancelRunSubmitting },
        ]}
        onClose={s.handleCloseCancelRunModal}
      />

      {isFuzzySearchOpen && (
        <FuzzySearchModal
          runs={s.runs}
          workflows={s.workflows}
          onSelectRun={handleFuzzySearchSelectRun}
          onSelectWorkflow={handleFuzzySearchSelectWorkflow}
          onClose={closeFuzzySearch}
        />
      )}
    </>
  );
}
