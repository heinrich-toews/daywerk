export type Role = "employee" | "office" | "admin";
export type AccountKind = "work" | "break";

export interface Account {
  id: string;
  org_id: string;
  name: string;
  kind: AccountKind;
  icon: string | null;
  sort_order: number;
  archived: boolean;
  is_primary: boolean;
}

export interface User {
  id: string;
  org_id: string;
  email: string;
  name: string;
  role: Role;
  locale: string;
  active: boolean;
  created_at: number;
}

export interface Org {
  id: string;
  name: string;
  slug: string;
  logo_path: string | null;
  color_primary: string;
  color_accent: string;
  timezone: string;
  default_weekly_hours: number;
  default_workdays_per_week: number;
}

export interface Segment {
  id: string;
  workday_id: string;
  user_id: string;
  account_id: string;
  started_at: number;
  ended_at: number | null;
  source: string;
  note: string;
  material: string;
}

export interface Workday {
  id: string;
  user_id: string;
  date: string;
  opened_at: number;
  closed_at: number | null;
  status: "open" | "closed";
}

export interface Totals {
  work_ms: number;
  break_ms: number;
  span_ms: number;
}

export interface DayView {
  day: Workday;
  segments: Segment[];
  totals: Totals;
  /** Target for this day, from the contract that applied then. */
  target_minutes: number;
}

export interface WorkTimeModel {
  id: string;
  user_id: string;
  valid_from: string;
  weekly_hours: number;
  workdays_per_week: number;
  note: string;
  created_at: number;
}

export interface Balance {
  worked_ms: number;
  break_ms: number;
  days: number;
  target_minutes: number;
  target_ms: number;
  adjustments_ms: number;
  balance_ms: number;
}

export interface ActiveInfo {
  account_id: string;
  account_name: string;
  kind: AccountKind;
  started_at: number;
}

export interface TodayData {
  now: number;
  date: string;
  user: User;
  org: Org;
  accounts: Account[];
  day: DayView | null;
  active: ActiveInfo | null;
  balance_month: Balance;
  balance_total: Balance;
  target_minutes: number;
  work_time_model: WorkTimeModel | null;
  outcome?: { outcome: string };
}

export interface Me {
  user: User;
  org: Org;
  accounts: Account[];
  balance_month: Balance;
  balance_total: Balance;
  target_minutes: number;
  work_time_model: WorkTimeModel | null;
}

export interface LoginResponse {
  token: string;
  user: User;
  org: Org;
  accounts: Account[];
}

export interface Stats {
  period: Balance;
  total: Balance;
  per_account: { account_id: string; account_name: string; kind: AccountKind; ms: number }[];
  per_day: { date: string; work_ms: number; break_ms: number }[];
}

export type TaskPriority = "low" | "normal" | "high" | "urgent";
export type TaskStatus = "open" | "in_progress" | "done" | "cancelled";

export interface Project {
  id: string;
  org_id: string;
  name: string;
  description: string;
  color: string | null;
  account_id: string | null;
  status: "active" | "archived";
  created_by: string | null;
  created_at: number;
}

export interface ProjectRow extends Project {
  account_name: string | null;
  task_count: number;
  open_count: number;
  done_count: number;
  overdue_count: number;
  estimate_minutes: number;
  spent_minutes: number;
  booked_ms: number;
}

export interface Task {
  id: string;
  project_id: string;
  parent_id: string | null;
  title: string;
  description: string;
  assignee_id: string | null;
  start_date: string | null;
  due_date: string | null;
  priority: TaskPriority;
  status: TaskStatus;
  estimate_minutes: number | null;
  created_by: string | null;
  created_at: number;
  updated_at: number;
  completed_at: number | null;
}

export interface TaskRow extends Task {
  project_name: string;
  assignee_name: string | null;
  subtask_count: number;
  done_subtasks: number;
  spent_minutes: number;
  comment_count: number;
}

export interface TaskComment {
  id: string;
  task_id: string;
  author_id: string | null;
  author_name: string;
  body: string;
  created_at: number;
}

export interface TaskTimeEntry {
  id: string;
  task_id: string;
  user_id: string | null;
  user_name: string;
  minutes: number;
  note: string;
  spent_on: string;
  created_at: number;
}

export interface ProjectDetail {
  project: ProjectRow;
  tasks: TaskRow[];
  accounts: { id: string; name: string; archived: boolean }[];
}

export interface TaskDetail {
  task: TaskRow;
  subtasks: Task[];
  comments: TaskComment[];
  time_entries: TaskTimeEntry[];
}

export interface Member {
  id: string;
  name: string;
}

export interface TaskStatsRow {
  id: string;
  name: string;
  task_count: number;
  done_count: number;
  overdue_count: number;
  estimate_minutes: number;
  spent_minutes: number;
  booked_ms: number;
}

export interface TaskStats {
  from: string;
  to: string;
  total_tasks: number;
  open_tasks: number;
  done_tasks: number;
  overdue_tasks: number;
  estimate_minutes: number;
  spent_minutes: number;
  booked_ms: number;
  by_project: TaskStatsRow[];
  by_assignee: TaskStatsRow[];
}

export interface BookingRow {
  id: string;
  date: string;
  user_id: string;
  user_name: string;
  account_id: string;
  account_name: string;
  started_at: number;
  ended_at: number | null;
  note: string;
  material: string;
  org_id: string;
  org_name: string;
}

export interface TeamRow {
  user: User;
  org_id: string;
  org_name: string;
  period: Balance;
  total: Balance;
  current_model: WorkTimeModel | null;
}

const TOKEN_KEY = "daywerk.token";
const SERVER_KEY = "daywerk.server";

/**
 * The web app is served by the Daywerk server, so relative `/api` paths work.
 * The native apps are not, so they need an absolute address. It comes from
 * `VITE_SERVER_URL` at build time (handy for a fixed deployment) and can be
 * changed at runtime on the login screen or in the settings.
 */
const BUILD_SERVER = (import.meta.env["VITE_SERVER_URL"] as string | undefined) ?? "";

function readStored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

let serverBase = (readStored(SERVER_KEY) ?? BUILD_SERVER).replace(/\/+$/, "");

export function getServerBase(): string {
  return serverBase;
}

/** Point the client at another server. The old session belongs to the old one. */
export function setServerBase(value: string) {
  const clean = value.trim().replace(/\/+$/, "");
  if (clean === serverBase) return;
  serverBase = clean;
  try {
    if (clean) localStorage.setItem(SERVER_KEY, clean);
    else localStorage.removeItem(SERVER_KEY);
  } catch {
    /* private mode */
  }
  setToken(null);
}

function apiUrl(path: string): string {
  return serverBase ? `${serverBase}${path}` : path;
}

/** True inside the Tauri shell (Android/iOS/desktop). */
export const isTauri =
  typeof window !== "undefined" &&
  (Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) ||
    window.location.hostname === "tauri.localhost");

export class ApiError extends Error {
  status: number;
  constructor(message: string, status: number) {
    super(message);
    this.status = status;
  }
}

let token: string | null = null;
try {
  token = localStorage.getItem(TOKEN_KEY);
} catch {
  token = null;
}

export function getToken(): string | null {
  return token;
}

export function setToken(next: string | null) {
  token = next;
  try {
    if (next) localStorage.setItem(TOKEN_KEY, next);
    else localStorage.removeItem(TOKEN_KEY);
  } catch {
    /* private mode */
  }
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body) headers.set("Content-Type", "application/json");
  if (token) headers.set("Authorization", `Bearer ${token}`);

  let res: Response;
  try {
    res = await fetch(apiUrl(path), { ...init, headers });
  } catch {
    throw new ApiError("network", 0);
  }

  if (res.status === 204) return undefined as T;

  const text = await res.text();
  let body: unknown = null;
  if (text) {
    try {
      body = JSON.parse(text);
    } catch {
      body = null;
    }
  }

  if (!res.ok) {
    const message =
      body && typeof body === "object" && "error" in body
        ? String((body as { error: unknown }).error)
        : res.statusText;
    throw new ApiError(message, res.status);
  }

  return body as T;
}

function qs(params: Record<string, string | number | boolean | undefined | null>): string {
  const search = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null || v === "") continue;
    search.set(k, String(v));
  }
  const s = search.toString();
  return s ? `?${s}` : "";
}

export const api = {
  login: (email: string, password: string) =>
    request<LoginResponse>("/api/auth/login", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    }),

  logout: () => request<void>("/api/auth/logout", { method: "POST" }),

  me: () => request<Me>("/api/me"),

  today: () => request<TodayData>("/api/today"),

  punch: (
    action: "start" | "pause" | "stop",
    accountId?: string,
    note?: string,
    material?: string,
  ) =>
    request<TodayData>("/api/punch", {
      method: "POST",
      body: JSON.stringify({ action, account_id: accountId, note, material }),
    }),

  updateSegmentNote: (id: string, note: string, material: string) =>
    request<Segment>(`/api/segments/${id}`, {
      method: "PATCH",
      body: JSON.stringify({ note, material }),
    }),

  bookings: (params: {
    q?: string;
    from?: string;
    to?: string;
    user_id?: string;
    account_id?: string;
    org_id?: string;
    limit?: number;
    offset?: number;
  }) => request<BookingRow[]>(`/api/bookings${qs(params)}`),

  days: (from: string, to: string, userId?: string) =>
    request<DayView[]>(`/api/days${qs({ from, to, user_id: userId })}`),

  stats: (from: string, to: string, userId?: string) =>
    request<Stats>(`/api/stats${qs({ from, to, user_id: userId })}`),

  team: (from: string, to: string, orgId?: string) =>
    request<TeamRow[]>(`/api/team${qs({ from, to, org_id: orgId })}`),

  users: (orgId?: string) => request<User[]>(`/api/users${qs({ org_id: orgId })}`),

  createUser: (body: {
    org_id?: string;
    email: string;
    name: string;
    password: string;
    role: Role;
    locale?: string;
    weekly_hours?: number;
    workdays_per_week?: number;
  }) => request<User>("/api/users", { method: "POST", body: JSON.stringify(body) }),

  updateUser: (
    id: string,
    body: {
      name?: string;
      role?: Role;
      locale?: string;
      active?: boolean;
      password?: string;
    },
  ) => request<User>(`/api/users/${id}`, { method: "PATCH", body: JSON.stringify(body) }),

  deleteUser: (id: string) => request<void>(`/api/users/${id}`, { method: "DELETE" }),

  workTimeModels: (userId: string) =>
    request<WorkTimeModel[]>(`/api/users/${userId}/work-time-models`),

  createWorkTimeModel: (
    userId: string,
    body: { valid_from: string; weekly_hours: number; workdays_per_week: number; note?: string },
  ) =>
    request<WorkTimeModel>(`/api/users/${userId}/work-time-models`, {
      method: "POST",
      body: JSON.stringify(body),
    }),

  updateWorkTimeModel: (
    id: string,
    body: { weekly_hours: number; workdays_per_week: number; note?: string },
  ) =>
    request<WorkTimeModel>(`/api/work-time-models/${id}`, {
      method: "PATCH",
      body: JSON.stringify(body),
    }),

  updateProfile: (body: { name?: string; locale?: string; password?: string }) =>
    request<User>("/api/profile", { method: "PUT", body: JSON.stringify(body) }),

  accounts: (orgId?: string) => request<Account[]>(`/api/accounts${qs({ org_id: orgId })}`),

  deleteAccount: (id: string) =>
    request<void>(`/api/accounts/${id}`, { method: "DELETE" }),

  createAccount: (body: {
    name: string;
    kind: AccountKind;
    icon?: string;
    sort_order?: number;
    is_primary?: boolean;
    org_id?: string;
  }) => request<Account>("/api/accounts", { method: "POST", body: JSON.stringify(body) }),

  updateAccount: (
    id: string,
    body: {
      name?: string;
      icon?: string | null;
      sort_order?: number;
      archived?: boolean;
      is_primary?: boolean;
    },
  ) => request<Account>(`/api/accounts/${id}`, { method: "PATCH", body: JSON.stringify(body) }),

  org: () => request<Org>("/api/org"),

  updateOrg: (body: {
    name: string;
    color_primary: string;
    color_accent: string;
    timezone: string;
    default_weekly_hours: number;
    default_workdays_per_week: number;
    logo_path?: string | null;
  }) => request<Org>("/api/org", { method: "PUT", body: JSON.stringify(body) }),

  orgs: () => request<Org[]>("/api/orgs"),

  createOrg: (body: {
    name: string;
    slug: string;
    timezone?: string;
    default_weekly_hours?: number;
    default_workdays_per_week?: number;
  }) => request<Org>("/api/orgs", { method: "POST", body: JSON.stringify(body) }),

  updateOrgById: (
    id: string,
    body: {
      name: string;
      color_primary: string;
      color_accent: string;
      timezone: string;
      default_weekly_hours: number;
      default_workdays_per_week: number;
      logo_path?: string | null;
    },
  ) => request<Org>(`/api/orgs/${id}`, { method: "PATCH", body: JSON.stringify(body) }),

  addAdjustment: (body: { user_id: string; date: string; minutes: number; reason?: string }) =>
    request<void>("/api/adjustments", { method: "POST", body: JSON.stringify(body) }),

  members: () => request<Member[]>("/api/members"),

  projects: (includeArchived = false) =>
    request<ProjectRow[]>(`/api/projects${qs({ include_archived: includeArchived })}`),

  project: (id: string) => request<ProjectDetail>(`/api/projects/${id}`),

  createProject: (body: {
    name: string;
    description?: string;
    color?: string | null;
    account_id?: string | null;
  }) => request<Project>("/api/projects", { method: "POST", body: JSON.stringify(body) }),

  updateProject: (
    id: string,
    body: {
      name?: string;
      description?: string;
      color?: string | null;
      account_id?: string;
      status?: "active" | "archived";
    },
  ) => request<Project>(`/api/projects/${id}`, { method: "PATCH", body: JSON.stringify(body) }),

  projectDeletePreview: (id: string) =>
    request<{
      tasks: number;
      subtasks: number;
      comments: number;
      minutes: number;
      account_name: string | null;
    }>(`/api/projects/${id}/preview`),

  deleteProject: (id: string) => request<void>(`/api/projects/${id}`, { method: "DELETE" }),

  tasks: (params: {
    project_id?: string;
    assignee_id?: string;
    status?: string;
    priority?: string;
    q?: string;
    mine?: boolean;
    limit?: number;
  }) => request<TaskRow[]>(`/api/tasks${qs(params)}`),

  task: (id: string) => request<TaskDetail>(`/api/tasks/${id}`),

  createTask: (body: {
    project_id: string;
    parent_id?: string | null;
    title: string;
    description?: string;
    assignee_id?: string | null;
    start_date?: string | null;
    due_date?: string | null;
    priority?: TaskPriority;
    estimate_minutes?: number | null;
  }) => request<Task>("/api/tasks", { method: "POST", body: JSON.stringify(body) }),

  updateTask: (
    id: string,
    body: {
      title: string;
      description?: string;
      assignee_id?: string | null;
      start_date?: string | null;
      due_date?: string | null;
      priority?: TaskPriority;
      status?: TaskStatus;
      estimate_minutes?: number | null;
    },
  ) => request<Task>(`/api/tasks/${id}`, { method: "PATCH", body: JSON.stringify(body) }),

  deleteTask: (id: string) => request<void>(`/api/tasks/${id}`, { method: "DELETE" }),

  addComment: (taskId: string, body: string) =>
    request<TaskComment>(`/api/tasks/${taskId}/comments`, {
      method: "POST",
      body: JSON.stringify({ body }),
    }),

  deleteComment: (taskId: string, commentId: string) =>
    request<void>(`/api/tasks/${taskId}/comments/${commentId}`, { method: "DELETE" }),

  addTaskTime: (
    taskId: string,
    body: { minutes: number; note?: string; spent_on: string },
  ) =>
    request<TaskTimeEntry>(`/api/tasks/${taskId}/time`, {
      method: "POST",
      body: JSON.stringify(body),
    }),

  deleteTaskTime: (taskId: string, entryId: string) =>
    request<void>(`/api/tasks/${taskId}/time/${entryId}`, { method: "DELETE" }),

  taskStats: (from?: string, to?: string) =>
    request<TaskStats>(`/api/tasks/stats${qs({ from, to })}`),

  exportBackup: () => request<unknown>("/api/export"),

  importBackup: (data: unknown) =>
    request<Record<string, number>>("/api/import", {
      method: "POST",
      body: JSON.stringify(data),
    }),
};

/** Download a timesheet through fetch so the bearer token stays out of the URL. */
export async function downloadTimesheet(
  kind: "xlsx" | "pdf",
  params: { from?: string; to?: string; org_id?: string; user_id?: string },
): Promise<void> {
  const headers = new Headers();
  if (token) headers.set("Authorization", `Bearer ${token}`);

  let res: Response;
  try {
    res = await fetch(apiUrl(`/api/export/timesheet.${kind}${qs(params)}`), { headers });
  } catch {
    throw new ApiError("network", 0);
  }

  if (!res.ok) {
    const text = await res.text();
    let message = res.statusText;
    try {
      const parsed = JSON.parse(text) as { error?: unknown };
      if (parsed.error) message = String(parsed.error);
    } catch {
      /* keep status text */
    }
    throw new ApiError(message, res.status);
  }

  const blob = await res.blob();
  const suffix = params.from && params.to ? `${params.from}_${params.to}` : "export";
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `daywerk-${suffix}.${kind}`;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

/** Download the JSON backup through fetch so the bearer token is included. */
export async function downloadBackup(): Promise<void> {
  const data = await api.exportBackup();
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `daywerk-backup-${new Date().toISOString().slice(0, 10)}.json`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}
