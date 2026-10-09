import type { MouseEvent as ReactMouseEvent } from 'react';

export type AppPage = 'main' | 'lineage' | 'stats';

export type Theme = 'dark' | 'light' | 'system';

const THEME_KEY = 'czar-react-theme';

export function readTheme(): Theme {
  if (typeof window === 'undefined') return 'system';
  try {
    const raw = window.localStorage.getItem(THEME_KEY);
    if (raw === 'dark' || raw === 'light' || raw === 'system') return raw;
  } catch {
    // ignore
  }
  return 'system';
}

export function saveTheme(theme: Theme): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(THEME_KEY, theme);
  } catch {
    // ignore persistence failures
  }
}

const LOG_DETAILS_KEY = 'czar_log_show_details';

export function readLogDetails(): boolean {
  try { return localStorage.getItem(LOG_DETAILS_KEY) === 'true'; } catch { return false; }
}

export function saveLogDetails(value: boolean): void {
  try { localStorage.setItem(LOG_DETAILS_KEY, String(value)); } catch { /* ignore */ }
}

export const WORKFLOWS_PER_PAGE = 50;
export const RUNS_PER_PAGE = 50;
export const WORKFLOW_GRID_MAX_RUNS = 200;
export const MOBILE_LAYOUT_MAX_WIDTH = 1100;
export const DEFAULT_SIDEBAR_WIDTH = 260;
export const MIN_SIDEBAR_WIDTH = 160;
export const MAX_SIDEBAR_WIDTH = 520;
export const SIDEBAR_COLLAPSE_THRESHOLD = 80;

const DESKTOP_SIDEBAR_STATE_KEY = 'czar-react-desktop-sidebar';

export interface DesktopSidebarState {
  width: number;
  lastExpandedWidth: number;
}

export function shouldHandleClientNavigation(event: ReactMouseEvent<HTMLAnchorElement>): boolean {
  return event.button === 0
    && !event.defaultPrevented
    && !event.metaKey
    && !event.ctrlKey
    && !event.shiftKey
    && !event.altKey;
}

export function readPageFromUrl(): AppPage {
  if (typeof window === 'undefined') return 'main';
  const page = new URLSearchParams(window.location.search).get('page');
  if (page === 'lineage') return 'lineage';
  if (page === 'stats') return 'stats';
  return 'main';
}

function clampDesktopSidebarWidth(width: number): number {
  return Math.min(MAX_SIDEBAR_WIDTH, Math.max(0, width));
}

function clampExpandedSidebarWidth(width: number): number {
  return Math.min(MAX_SIDEBAR_WIDTH, Math.max(MIN_SIDEBAR_WIDTH, width));
}

export function readDesktopSidebarState(): DesktopSidebarState {
  if (typeof window === 'undefined') {
    return {
      width: DEFAULT_SIDEBAR_WIDTH,
      lastExpandedWidth: DEFAULT_SIDEBAR_WIDTH,
    };
  }

  try {
    const raw = window.localStorage.getItem(DESKTOP_SIDEBAR_STATE_KEY);
    if (!raw) {
      return {
        width: DEFAULT_SIDEBAR_WIDTH,
        lastExpandedWidth: DEFAULT_SIDEBAR_WIDTH,
      };
    }

    const parsed = JSON.parse(raw) as Partial<DesktopSidebarState>;
    const width = typeof parsed.width === 'number'
      ? clampDesktopSidebarWidth(parsed.width)
      : DEFAULT_SIDEBAR_WIDTH;
    const lastExpandedWidth = typeof parsed.lastExpandedWidth === 'number'
      ? clampExpandedSidebarWidth(parsed.lastExpandedWidth)
      : DEFAULT_SIDEBAR_WIDTH;

    return { width, lastExpandedWidth };
  } catch {
    return {
      width: DEFAULT_SIDEBAR_WIDTH,
      lastExpandedWidth: DEFAULT_SIDEBAR_WIDTH,
    };
  }
}

export function saveDesktopSidebarState(state: DesktopSidebarState): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(DESKTOP_SIDEBAR_STATE_KEY, JSON.stringify(state));
  } catch {
    // ignore persistence failures
  }
}
