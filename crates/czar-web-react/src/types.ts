export interface Workflow {
  name: string;
  schedule?: string;
  schedule_enabled: boolean;
  next_scheduled_at?: string;
}

export interface WorkflowsResponse {
  items: Workflow[];
  total: number;
}

export interface Run {
  id: string;
  workflow: string;
  status: string;
  task_selection?: RunTaskSelection;
  created_at: string;
  started_at?: string;
  finished_at?: string;
  error?: string;
}

export interface RunsResponse {
  items: Run[];
  total: number;
}

export interface Task {
  id: string;
  name: string;
  status: string;
  executor: string;
  depends_on: string[];
  started_at?: string;
  finished_at?: string;
  dispatched_at?: string;
  output?: unknown;
  logs?: string;
  error?: string;
  progress?: TaskProgress;
}

export interface TaskProgress {
  current?: number;
  total?: number;
  message?: string;
}

export interface WorkflowTaskInfo {
  name: string;
  depends_on: string[];
  executor: string;
  params?: Record<string, unknown>;
  last_state?: WorkflowTaskLastStateInfo;
}

export interface GraphNode {
  id: string;
  deps: string[];
  label?: string;
  exportLabel?: string;
}

export interface WorkflowInfo {
  name: string;
  tasks: WorkflowTaskInfo[];
  task_groups: Record<string, string[]>;
  flows: string[];
  schedule?: string;
  schedule_enabled: boolean;
  next_scheduled_at?: string;
  source_yaml?: string;
}

export interface RunDetail {
  run: Run;
  tasks: Task[];
  workflow?: WorkflowInfo;
}

export interface CreateRunResponse {
  run_id: string;
}

export interface ReloadWorkflowDefinitionsResult {
  file: string;
  workflow?: string;
  outcome: string;
  message?: string;
}

export interface ReloadWorkflowDefinitionsResponse {
  definitions_root: string;
  discovered: number;
  reloaded: number;
  skipped: number;
  failed: number;
  results: ReloadWorkflowDefinitionsResult[];
}

export interface WorkflowDefinitionsConfigResponse {
  definitions_root: string;
}

export interface WorkflowDetail {
  workflow: WorkflowInfo;
  tasks: WorkflowTaskInfo[];
}

export type TaskSelectionMode = 'exact' | 'upstream' | 'downstream' | 'full';

export interface RunTaskSelection {
  mode: TaskSelectionMode;
  flow_name?: string;
  requested_tasks: string[];
  requested_groups: string[];
  resolved_tasks: string[];
}

export interface StartRunSelectionInput {
  task_names?: string[];
  group_names?: string[];
  mode?: TaskSelectionMode;
}

export interface TriggerRunRequestOptions {
  flow?: string;
  selection?: StartRunSelectionInput;
}

export interface WorkflowTaskStateListItem {
  task_name: string;
  last_status?: string;
  last_run_id?: string;
  last_task_id?: string;
  run_api_path?: string;
  finished_at?: string;
  success_at?: string;
  failure_at?: string;
  output?: unknown;
  error?: string;
}

export interface WorkflowTaskStateListResponse {
  workflow: string;
  items: WorkflowTaskStateListItem[];
}

export interface WorkflowTaskLastStateInfo {
  status: string;
  run_id: string;
  task_id: string;
  finished_at?: string;
  success_at?: string;
  failure_at?: string;
  output?: unknown;
  error?: string;
}
