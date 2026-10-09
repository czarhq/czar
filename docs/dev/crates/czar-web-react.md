# czar-web-react

_Updated at: 2026-05-22_

Frontend crate for the react framework implementation.

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [index.html](../../../crates/czar-web-react/index.html) | Vite HTML entry shell that mounts the React app and loads the bundled frontend assets. | 2026-03-15 | 9456793 |
| [pnpm-lock.yaml](../../../crates/czar-web-react/pnpm-lock.yaml) | pnpm lockfile pinning the React app's frontend toolchain and UI dependencies. | 2026-03-29 | 3e39880 |
| [package.json](../../../crates/czar-web-react/package.json) | React web app manifest with Vite scripts and frontend dependency declarations. | 2026-03-15 | 75b40d4 |
| [public/config.js](../../../crates/czar-web-react/public/config.js) | Runtime config shim for frontend deployments that need to inject base URLs or environment-specific values at asset-serve time. | 2026-03-15 | dbb479f |
| [src/App.tsx](../../../crates/czar-web-react/src/App.tsx) | Top-level React app component wiring sidebar state, selection URL sync, polling, reload actions, and detail-panel composition. | 2026-03-15 | 88a91f1 |
| [src/api/index.ts](../../../crates/czar-web-react/src/api/index.ts) | Frontend API client for workflows, runs, task states, and reload actions with shared request/error handling. | 2026-03-15 | 0bfec71 |
| [src/appState.ts](../../../crates/czar-web-react/src/appState.ts) | Shared app-level constants and URL/sidebar state helpers used by the top-level React shell. | 2026-03-15 | 6993261 |
| [src/components/AppLayout.tsx](../../../crates/czar-web-react/src/components/AppLayout.tsx) | Primary page layout that arranges navigation, sidebar panels, toolbar actions, and the main detail surface. | 2026-03-15 | dd358c1 |
| [src/components/DetailPanel.tsx](../../../crates/czar-web-react/src/components/DetailPanel.tsx) | Main detail-surface switchboard for run detail, workflow graph/grid views, and execution controls. | 2026-03-15 | 307b5b0 |
| [src/components/DetailPanelExecutionToolbar.tsx](../../../crates/czar-web-react/src/components/DetailPanelExecutionToolbar.tsx) | Execution controls with run configuration and task selection options. | 2026-03-12 | e96f69b |
| [src/components/DetailPanelGraphGrid.tsx](../../../crates/czar-web-react/src/components/DetailPanelGraphGrid.tsx) | Combined workflow/run graph and run-grid presentation surface with selection-aware controls and empty states. | 2026-03-15 | ed92224 |
| [src/components/EnvBanner.tsx](../../../crates/czar-web-react/src/components/EnvBanner.tsx) | Small environment banner for surfacing deployment-mode notices or constrained runtime warnings in the UI. | 2026-03-15 | 91efaca |
| [src/components/Graph.tsx](../../../crates/czar-web-react/src/components/Graph.tsx) | React Flow-based DAG renderer for workflow and run task graphs, including overlays, statuses, and selection affordances. | 2026-03-15 | 42938ff |
| [src/components/GraphFitViewOnChange.tsx](../../../crates/czar-web-react/src/components/GraphFitViewOnChange.tsx) | React Flow helper that calls `fitView` when graph trigger keys change after nodes finish initialization. | 2026-03-12 | f4f909e |
| [src/components/GraphMiniPreview.tsx](../../../crates/czar-web-react/src/components/GraphMiniPreview.tsx) | SVG minimap overlay rendering scaled node rectangles and edges for the current graph. | 2026-03-03 | 550de66 |
| [src/components/LineagePage.tsx](../../../crates/czar-web-react/src/components/LineagePage.tsx) | Workflow lineage page summarizing graph relationships and materialization state outside the main run-focused layout. | 2026-03-15 | 69b9081 |
| [src/components/MessageModal.tsx](../../../crates/czar-web-react/src/components/MessageModal.tsx) | Reusable modal dialog for confirmations and one-off status/error messages. | 2026-03-12 | e7ea4d5 |
| [src/components/Pagination.tsx](../../../crates/czar-web-react/src/components/Pagination.tsx) | Prev/Next pagination control that renders only when total pages exceeds one. | 2026-02-24 | 05ce17e |
| [src/components/ReloadDefinitionsModal.tsx](../../../crates/czar-web-react/src/components/ReloadDefinitionsModal.tsx) | Modal presenting the outcome of workflow-definition reload operations. | 2026-03-12 | fc8b5c3 |
| [src/components/RunConfigurationBody.tsx](../../../crates/czar-web-react/src/components/RunConfigurationBody.tsx) | Configuration UI for task selection modes and run parameters. | 2026-03-12 | b9f9b43 |
| [src/components/RunDetailView.tsx](../../../crates/czar-web-react/src/components/RunDetailView.tsx) | Detailed run/task inspector showing task states, logs, outputs, and run metadata. | 2026-03-15 | 410cf14 |
| [src/components/RunGanttView.tsx](../../../crates/czar-web-react/src/components/RunGanttView.tsx) | Timeline-style gantt visualization for run task execution order and durations. | 2026-03-15 | 73872ac |
| [src/components/RunsPanel.tsx](../../../crates/czar-web-react/src/components/RunsPanel.tsx) | Sidebar panel listing runs with status/workflow filters, pagination, and selection highlight. | 2026-03-12 | cf9a8fa |
| [src/components/SidebarPanels.tsx](../../../crates/czar-web-react/src/components/SidebarPanels.tsx) | Sidebar tab container for workflow and run lists, filters, pagination, and selection state. | 2026-03-15 | 9c64814 |
| [src/components/StatsPage.tsx](../../../crates/czar-web-react/src/components/StatsPage.tsx) | Stats-oriented page for summarized workflow and run information outside the standard split-pane layout. | 2026-03-15 | 8f23e50 |
| [src/components/TaskMaterializationPanel.tsx](../../../crates/czar-web-react/src/components/TaskMaterializationPanel.tsx) | Task last-materialization table with task/status filters, focus-to-graph actions, and virtualization for large workflows. | 2026-03-12 | edcb842 |
| [src/components/WorkflowPanel.tsx](../../../crates/czar-web-react/src/components/WorkflowPanel.tsx) | Sidebar workflow list with search, pagination, latest-run badges, and selection handling. | 2026-03-15 | 54b5799 |
| [src/components/WorkflowRunsGrid.tsx](../../../crates/czar-web-react/src/components/WorkflowRunsGrid.tsx) | Workflow run-vs-task status matrix with selectable cells and virtualized task rows for large grids. | 2026-03-12 | d4b5677 |
| [src/components/WorkflowRunsList.tsx](../../../crates/czar-web-react/src/components/WorkflowRunsList.tsx) | Inline scrollable run list below the graph showing run IDs, statuses, and timestamps. | 2026-02-26 | c3f75c8 |
| [src/components/WorkflowYamlPanel.tsx](../../../crates/czar-web-react/src/components/WorkflowYamlPanel.tsx) | Collapsible panel rendering the workflow source YAML in a preformatted block. | 2026-03-12 | db859ce |
| [src/components/detailPanelUtils.ts](../../../crates/czar-web-react/src/components/detailPanelUtils.ts) | Normalizes and deduplicates run task names and signatures. | 2026-03-12 | ff6354f |
| [src/components/graphUtils.ts](../../../crates/czar-web-react/src/components/graphUtils.ts) | Graph layout and node/edge transformation helpers shared by workflow and run graph views. | 2026-03-15 | 3b2ebae |
| [src/components/selectionUtils.ts](../../../crates/czar-web-react/src/components/selectionUtils.ts) | Graph traversal helpers for resolving upstream/downstream/full task selection sets. | 2026-03-03 | 717d737 |
| [src/components/useDetailPanelScope.ts](../../../crates/czar-web-react/src/components/useDetailPanelScope.ts) | Derives the active graph/grid/detail scope for the detail panel from selected run, workflow history, and task-set signatures. | 2026-03-12 | 6f0fc8f |
| [src/components/useDetailPanelSelection.ts](../../../crates/czar-web-react/src/components/useDetailPanelSelection.ts) | Encapsulates detail-panel task/group selection state, state-filter application, and preview resolution for workflow runs. | 2026-03-12 | 98990e7 |
| [src/hooks/useAppData.ts](../../../crates/czar-web-react/src/hooks/useAppData.ts) | Shared top-level data-loading hook for workflows, runs, workflow detail, task states, and load-error state. | 2026-03-15 | 5883960 |
| [src/hooks/useAppHrefs.ts](../../../crates/czar-web-react/src/hooks/useAppHrefs.ts) | Memoized helper hook for building page, sidebar, and run-detail navigation URLs from current app selection state. | 2026-03-15 | d771367 |
| [src/hooks/useGraphPresentationData.ts](../../../crates/czar-web-react/src/hooks/useGraphPresentationData.ts) | Derives graph nodes, status maps, progress maps, and grid task metadata from selected run/workflow data. | 2026-03-12 | 0ee0951 |
| [src/hooks/useMobileSidebar.ts](../../../crates/czar-web-react/src/hooks/useMobileSidebar.ts) | Manages mobile layout detection and sidebar open/close state. | 2026-03-12 | 9fa9828 |
| [src/hooks/usePollCadence.ts](../../../crates/czar-web-react/src/hooks/usePollCadence.ts) | Polling cadence hook that adapts refresh intervals to focus/visibility and local-development conditions. | 2026-03-15 | 13ae92d |
| [src/hooks/useRunDetailPolling.ts](../../../crates/czar-web-react/src/hooks/useRunDetailPolling.ts) | Run-detail polling hook with single-flight behavior, backoff, and terminal-state refresh coordination. | 2026-03-15 | 90b4adb |
| [src/hooks/useVirtualRows.ts](../../../crates/czar-web-react/src/hooks/useVirtualRows.ts) | Simple row virtualization hook for large scrolling tables and workflow grids. | 2026-03-15 | 78a3a4e |
| [src/hooks/useWorkflowRunData.ts](../../../crates/czar-web-react/src/hooks/useWorkflowRunData.ts) | React hook for workflow-centric run history and latest-run summary data used by the sidebar and grid views. | 2026-03-15 | 477a0fd |
| [src/main.tsx](../../../crates/czar-web-react/src/main.tsx) | React app entry point that mounts the App component into the DOM root. | 2026-03-14 | a0e4198 |
| [src/selectionUrl.ts](../../../crates/czar-web-react/src/selectionUrl.ts) | URL parsing/building helpers that preserve workflow, run, tab, and page selection across refreshes and deep links. | 2026-03-15 | 6b286a1 |
| [src/style.css](../../../crates/czar-web-react/src/style.css) | Aggregated stylesheet entrypoint that imports the app’s split CSS modules and shared theme rules. | 2026-03-15 | 4b495fb |
| [src/styles/base.css](../../../crates/czar-web-react/src/styles/base.css) | Base variables, resets, typography, and shared utility styles for the React UI. | 2026-03-15 | bf8fc37 |
| [src/styles/detail-panel.css](../../../crates/czar-web-react/src/styles/detail-panel.css) | Styles for the detail panel, task inspectors, and run/workflow subviews. | 2026-03-15 | 9eecb56 |
| [src/styles/execution.css](../../../crates/czar-web-react/src/styles/execution.css) | Styles for execution controls, run configuration UI, and task-state presentation. | 2026-03-15 | 86c0ad0 |
| [src/styles/layout.css](../../../crates/czar-web-react/src/styles/layout.css) | Overall shell layout, split panes, sidebars, headers, and structural spacing styles. | 2026-03-15 | c6f0bb7 |
| [src/styles/pages.css](../../../crates/czar-web-react/src/styles/pages.css) | Page-specific styles for alternate top-level views such as lineage and stats. | 2026-03-15 | 9ab54a7 |
| [src/styles/responsive.css](../../../crates/czar-web-react/src/styles/responsive.css) | Responsive breakpoints and mobile-specific layout/style overrides. | 2026-03-15 | dc17cde |
| [src/styles/workflow-grid.css](../../../crates/czar-web-react/src/styles/workflow-grid.css) | Styles for the workflow run grid, matrix cells, legends, and scroll containers. | 2026-03-15 | 2d99843 |
| [src/types.ts](../../../crates/czar-web-react/src/types.ts) | Shared frontend TypeScript types for workflows, runs, task states, reload responses, and API payloads. | 2026-03-15 | 62eceff |
| [src/utils/index.ts](../../../crates/czar-web-react/src/utils/index.ts) | Date/time formatting, duration calculation, status CSS class mapping, and string truncation helpers. | 2026-02-26 | c9884b0 |
| [tsconfig.json](../../../crates/czar-web-react/tsconfig.json) | TypeScript compiler config targeting ES2020 with strict mode and no-emit (Vite handles compilation). | 2026-02-24 | 109f0ac |
| [vite.config.ts](../../../crates/czar-web-react/vite.config.ts) | Vite config for the official React frontend, including API proxy and base-path handling. | 2026-03-14 | e5b1404 |
