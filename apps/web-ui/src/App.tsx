import { For, Match, Show, Switch, onMount, type JSX } from "solid-js";
import { boot, booting, me, roleAtLeast, setView, signOut, view, type View } from "./state";
import { t } from "./i18n";
import Login from "./views/Login";
import Today from "./views/Today";
import History from "./views/History";
import Stats from "./views/Stats";
import Team from "./views/Team";
import Settings from "./views/Settings";
import Admin from "./views/Admin";
import {
  ChartIcon,
  ClockIcon,
  CogIcon,
  ListIcon,
  LogoutIcon,
  UsersIcon,
} from "./components/Icons";

type Tab = { id: View; label: string; icon: () => JSX.Element };

function tabs(): Tab[] {
  const list: Tab[] = [
    { id: "today", label: t("nav.today"), icon: () => <ClockIcon /> },
    { id: "history", label: t("nav.history"), icon: () => <ListIcon /> },
    { id: "stats", label: t("nav.stats"), icon: () => <ChartIcon /> },
  ];
  if (roleAtLeast("buero")) {
    list.push({ id: "team", label: t("nav.team"), icon: () => <UsersIcon /> });
  }
  list.push({ id: "settings", label: t("nav.settings"), icon: () => <CogIcon /> });
  return list;
}

function Splash() {
  return (
    <div class="flex min-h-screen items-center justify-center">
      <div class="animate-pulse font-display text-2xl font-bold uppercase tracking-widest text-primary">
        Daywerk
      </div>
    </div>
  );
}

function Shell() {
  const user = () => me()?.user;
  const org = () => me()?.org;

  return (
    <div class="flex min-h-screen flex-col pb-24">
      <header class="sticky top-0 z-20 border-b border-line bg-card/95 backdrop-blur">
        <div class="mx-auto flex max-w-2xl items-center justify-between gap-2 px-4 py-2.5">
          <div class="min-w-0">
            <div class="truncate font-display text-lg font-bold uppercase leading-none tracking-wide">
              {org()?.name ?? "Daywerk"}
            </div>
            <div class="truncate text-xs text-muted">
              {user()?.name}
              <Show when={user()?.role !== "allgemein"}>
                <span class="ml-1 rounded bg-soft px-1.5 py-0.5 text-[10px] font-semibold uppercase text-primary">
                  {t(`admin.role.${user()?.role}` as "admin.role.allgemein")}
                </span>
              </Show>
            </div>
          </div>
          <button
            type="button"
            class="rounded-lg p-2 text-muted hover:bg-soft hover:text-foreground"
            title={t("settings.logout")}
            onClick={() => void signOut()}
          >
            <LogoutIcon />
          </button>
        </div>
      </header>

      <main class="mx-auto w-full max-w-2xl flex-1 px-4 py-4">
        <Switch>
          <Match when={view() === "today"}>
            <Today />
          </Match>
          <Match when={view() === "history"}>
            <History />
          </Match>
          <Match when={view() === "stats"}>
            <Stats />
          </Match>
          <Match when={view() === "team"}>
            <Team />
          </Match>
          <Match when={view() === "settings"}>
            <Settings />
          </Match>
          <Match when={view() === "admin"}>
            <Admin />
          </Match>
        </Switch>
      </main>

      <nav class="fixed inset-x-0 bottom-0 z-20 border-t border-line bg-card/95 pb-[env(safe-area-inset-bottom)] backdrop-blur">
        <div class="mx-auto flex max-w-2xl items-stretch justify-around px-1 py-1">
          <For each={tabs()}>
            {(tab) => (
              <button
                type="button"
                onClick={() => setView(tab.id)}
                class={`flex flex-1 flex-col items-center gap-0.5 rounded-lg px-1 py-1.5 text-[10px] font-semibold uppercase tracking-wide transition-colors ${
                  view() === tab.id ? "text-primary" : "text-muted hover:text-foreground"
                }`}
              >
                {tab.icon()}
                <span>{tab.label}</span>
              </button>
            )}
          </For>
        </div>
      </nav>
    </div>
  );
}

export default function App() {
  onMount(() => {
    void boot();
  });

  return (
    <Show when={!booting()} fallback={<Splash />}>
      <Show when={me()} fallback={<Login />}>
        <Shell />
      </Show>
    </Show>
  );
}
