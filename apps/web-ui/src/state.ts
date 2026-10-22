import { createSignal } from "solid-js";
import { api, ApiError, getToken, setToken, type Me, type Org, type TodayData } from "./api";
import { setLang, type Lang } from "./i18n";

/** The two worlds of the app. `time` is the punch clock, `tasks` the task board. */
export type World = "time" | "tasks";
export type TimeView = "today" | "history" | "search" | "stats" | "team";
export type TaskView = "projects" | "tasks" | "search" | "stats";
/** Views that belong to neither world (header gear). */
export type GlobalView = "settings" | "admin";

/** Kept for imports that still say `View`. */
export type View = TimeView | GlobalView;

export const [me, setMe] = createSignal<Me | null>(null);
export const [today, setToday] = createSignal<TodayData | null>(null);
/**
 * Local clock reading at the moment the current `today` payload arrived.
 *
 * The server sends `now` with every payload. The difference between that value
 * and the local clock when it *arrived* is the clock offset, and it must be
 * derived from this timestamp — not from `Date.now()` at render time, or the
 * offset drifts every time the view remounts and the running timer appears to
 * restart.
 */
export const [todayReceivedAt, setTodayReceivedAt] = createSignal(0);

let todaySeq = 0;

function applyToday(data: TodayData) {
  todaySeq += 1;
  setToday(data);
  setTodayReceivedAt(Date.now());
}
export const [world, setWorld] = createSignal<World>("time");
export const [view, setView] = createSignal<TimeView>("today");
export const [taskView, setTaskView] = createSignal<TaskView>("projects");
export const [globalView, setGlobalView] = createSignal<GlobalView | null>(null);
export const [openProjectId, setOpenProjectId] = createSignal<string | null>(null);

/**
 * Navigation stack for tasks, so a subtask knows its parent.
 * `[task]` from a list, `[task, subtask]` when descending.
 */
export const [taskPath, setTaskPath] = createSignal<string[]>([]);
export const openTaskId = () => taskPath().at(-1) ?? null;

export function openProject(id: string) {
  setOpenProjectId(id);
  setTaskPath([]);
}

/** Open a task from a list: start a fresh path. */
export function openTask(id: string) {
  setTaskPath([id]);
}

/** Descend into a subtask: keep the parent on the stack. */
export function openSubtask(id: string) {
  setTaskPath((path) => [...path, id]);
}

export function closeTask() {
  setTaskPath((path) => path.slice(0, -1));
}

export function closeProject() {
  setOpenProjectId(null);
  setTaskPath([]);
}

export function goTaskView(next: TaskView) {
  setTaskView(next);
  closeProject();
}

/** Switch worlds, leaving the other world on its last view. */
export function switchWorld(next: World) {
  setGlobalView(null);
  setWorld(next);
}

export function openSettings() {
  setGlobalView(globalView() === "settings" ? null : "settings");
}
export const [booting, setBooting] = createSignal(true);
export const [busy, setBusy] = createSignal(false);
export const [notice, setNotice] = createSignal<string | null>(null);

const RANK = { employee: 0, office: 1, admin: 2 } as const;

export function roleAtLeast(role: "employee" | "office" | "admin"): boolean {
  const current = me()?.user.role;
  if (!current) return false;
  return RANK[current] >= RANK[role];
}

export function applyTheme(org: Org) {
  const root = document.documentElement;
  if (org.color_primary) root.style.setProperty("--dw-brand", org.color_primary);
  if (org.color_accent) root.style.setProperty("--dw-soft", org.color_accent);
}

export async function boot() {
  setBooting(true);
  try {
    if (getToken()) {
      const current = await api.me();
      setMe(current);
      applyTheme(current.org);
      setLang((current.user.locale as Lang) ?? "de");
      applyToday(await api.today());
    }
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) {
      setToken(null);
      setMe(null);
    }
  } finally {
    setBooting(false);
  }
}

export async function signIn(email: string, password: string) {
  const result = await api.login(email, password);
  setToken(result.token);
  const current = await api.me();
  setMe(current);
  applyTheme(current.org);
  setLang((current.user.locale as Lang) ?? "de");
  applyToday(await api.today());
  setView("today");
  setWorld("time");
  setGlobalView(null);
}

export async function signOut() {
  try {
    await api.logout();
  } catch {
    /* token may already be invalid */
  }
  setToken(null);
  setMe(null);
  setToday(null);
  setTodayReceivedAt(0);
  todaySeq += 1;
  setView("today");
  setWorld("time");
  setGlobalView(null);
}

export async function punch(
  action: "start" | "pause" | "stop",
  accountId?: string,
  note?: string,
  material?: string,
) {
  setBusy(true);
  try {
    const data = await api.punch(action, accountId, note, material);
    applyToday(data);
    return data;
  } finally {
    setBusy(false);
  }
}

export async function refresh() {
  const current = await api.me();
  setMe(current);
  applyTheme(current.org);
  applyToday(await api.today());
}

/**
 * Fetch the day again. A punch that happens while this request is in flight
 * wins: its result bumps the sequence and the stale response is discarded.
 */
export async function reloadToday() {
  const before = todaySeq;
  const data = await api.today();
  if (before === todaySeq) applyToday(data);
}

export async function changeLocale(locale: Lang) {
  const user = await api.updateProfile({ locale });
  setLang(locale);
  const current = me();
  if (current) setMe({ ...current, user });
}
