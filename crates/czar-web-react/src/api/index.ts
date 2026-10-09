import type {
  WorkflowsResponse,
  RunsResponse,
  RunDetail,
  CreateRunResponse,
  WorkflowInfo,
  WorkflowTaskStateListResponse,
  ReloadWorkflowDefinitionsResponse,
  TriggerRunRequestOptions,
  WorkflowDefinitionsConfigResponse,
} from '../types';

/** Fired when the API answers 401: the UI shows its login page. */
export const UNAUTHORIZED_EVENT = 'czar:unauthorized';

export interface AuthStatus {
  /** `token` servers log the browser in here. */
  mode: 'none' | 'token';
  authenticated: boolean;
}

export async function fetchAuthStatus(): Promise<AuthStatus> {
  const res = await fetch('/api/auth/status', { cache: 'no-store' });
  return parseJson<AuthStatus>(res);
}

/** Trade the server's API token for a session cookie. Returns false for a wrong token. */
export async function login(token: string): Promise<boolean> {
  const res = await fetch('/api/auth/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ token }),
  });
  if (res.status === 401) return false;
  if (!res.ok) throw new Error(`Login failed: ${res.status} ${res.statusText}`);
  return true;
}

async function parseJson<T>(res: Response): Promise<T> {
  if (!res.ok) {
    const body = await res.text();
    const message = body.trim();
    throw new Error(
      message
        ? `Request failed: ${res.status} ${res.statusText}\n\n${message}`
        : `Request failed: ${res.status} ${res.statusText}`
    );
  }
  return res.json() as Promise<T>;
}

export async function fetchWorkflows(limit: number, offset: number, search: string): Promise<WorkflowsResponse> {
  let url = `/api/workflows?limit=${limit}&offset=${offset}`;
  if (search) url += `&search=${encodeURIComponent(search)}`;
  const res = await apiFetch(url);
  return parseJson<WorkflowsResponse>(res);
}

export async function fetchRuns(
  limit: number,
  offset: number,
  search: string,
  workflowFilter: string[],
  statusFilter: string[],
  dateFilter?: { from: string; to: string },
): Promise<RunsResponse> {
  let url = `/api/runs?limit=${limit}&offset=${offset}`;
  if (search) url += `&search=${encodeURIComponent(search)}`;
  if (workflowFilter.length > 0) url += `&workflow=${encodeURIComponent(workflowFilter.join(','))}`;
  if (statusFilter.length > 0) url += `&status=${encodeURIComponent(statusFilter.join(','))}`;
  if (dateFilter?.from) url += `&since=${encodeURIComponent(dateFilter.from)}`;
  if (dateFilter?.to) url += `&until=${encodeURIComponent(dateFilter.to)}`;
  const res = await apiFetch(url);
  return parseJson<RunsResponse>(res);
}

export async function fetchWorkflowDetail(name: string): Promise<WorkflowInfo> {
  const res = await apiFetch(`/api/workflows/${encodeURIComponent(name)}`);
  return parseJson<WorkflowInfo>(res);
}

export async function fetchWorkflowTaskStates(name: string): Promise<WorkflowTaskStateListResponse> {
  const res = await apiFetch(`/api/workflows/${encodeURIComponent(name)}/task-states`);
  return parseJson<WorkflowTaskStateListResponse>(res);
}

export async function fetchRunDetail(runId: string, slim = false): Promise<RunDetail> {
  const url = slim ? `/api/runs/${runId}?slim=true` : `/api/runs/${runId}`;
  const res = await apiFetch(url, { cache: 'no-store' });
  return parseJson<RunDetail>(res);
}

export async function triggerRun(
  workflow: string,
  options?: TriggerRunRequestOptions,
): Promise<CreateRunResponse> {
  const payload: Record<string, unknown> = { workflow };
  if (options?.flow) payload.flow = options.flow;
  if (options?.selection) payload.selection = options.selection;
  const res = await apiFetch('/api/runs', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  return parseJson<CreateRunResponse>(res);
}

export async function reloadWorkflowDefinitions(): Promise<ReloadWorkflowDefinitionsResponse> {
  const res = await apiFetch('/api/workflows/reload', { method: 'POST' });
  return parseJson<ReloadWorkflowDefinitionsResponse>(res);
}

export async function fetchWorkflowDefinitionsConfig(): Promise<WorkflowDefinitionsConfigResponse> {
  const res = await apiFetch('/api/workflows/definitions-root');
  return parseJson<WorkflowDefinitionsConfigResponse>(res);
}

export async function cancelRun(runId: string): Promise<void> {
  const res = await apiFetch(`/api/runs/${runId}/cancel`, { method: 'POST' });
  if (!res.ok) throw new Error(`Cancel failed: ${res.status} ${res.statusText}`);
}

export async function pauseRun(runId: string): Promise<void> {
  const res = await apiFetch(`/api/runs/${runId}/pause`, { method: 'POST' });
  if (!res.ok) throw new Error(`Pause failed: ${res.status} ${res.statusText}`);
}

export async function resumeRun(runId: string): Promise<void> {
  const res = await apiFetch(`/api/runs/${runId}/resume`, { method: 'POST' });
  if (!res.ok) throw new Error(`Resume failed: ${res.status} ${res.statusText}`);
}

// Helpers

async function apiFetch(input: string, init?: RequestInit): Promise<Response> {
  const res = await fetch(input, init);
  if (res.status === 401) window.dispatchEvent(new Event(UNAUTHORIZED_EVENT));
  return res;
}
