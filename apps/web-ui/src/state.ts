import { createSignal } from "solid-js";
import { api, ApiError, getToken, setToken, type Me, type Org, type TodayData } from "./api";
import { setLang, type Lang } from "./i18n";

export type View = "today" | "history" | "stats" | "team" | "settings" | "admin";

export const [me, setMe] = createSignal<Me | null>(null);
export const [today, setToday] = createSignal<TodayData | null>(null);
export const [view, setView] = createSignal<View>("today");
export const [booting, setBooting] = createSignal(true);
export const [busy, setBusy] = createSignal(false);
export const [notice, setNotice] = createSignal<string | null>(null);

const RANK = { allgemein: 0, buero: 1, admin: 2 } as const;

export function roleAtLeast(role: "allgemein" | "buero" | "admin"): boolean {
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
      setToday(await api.today());
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
  setToday(await api.today());
  setView("today");
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
  setView("today");
}

export async function punch(action: "start" | "pause" | "stop", accountId?: string) {
  setBusy(true);
  try {
    const data = await api.punch(action, accountId);
    setToday(data);
    return data;
  } finally {
    setBusy(false);
  }
}

export async function refresh() {
  const current = await api.me();
  setMe(current);
  applyTheme(current.org);
  setToday(await api.today());
}

export async function changeLocale(locale: Lang) {
  const user = await api.updateProfile({ locale });
  setLang(locale);
  const current = me();
  if (current) setMe({ ...current, user });
}
