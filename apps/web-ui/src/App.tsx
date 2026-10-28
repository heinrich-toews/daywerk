import { For, Match, Show, Switch, onMount } from "solid-js";
import type { JSX } from "solid-js";
import {
  boot,
  booting,
  globalView,
  goTaskView,
  me,
  openSettings,
  roleAtLeast,
  setGlobalView,
  setView,
  signOut,
  switchWorld,
  taskView,
  view,
  world,
} from "./state";
import { assetUrl } from "./api";
import { t } from "./i18n";
import Login from "./views/Login";
import Today from "./views/Today";
import History from "./views/History";
import Stats from "./views/Stats";
import Team from "./views/Team";
import Search from "./views/Search";
import Settings from "./views/Settings";
import Admin from "./views/Admin";
import TaskWorld from "./views/tasks/TaskWorld";
import { CogIcon, LogoutIcon } from "./components/Icons";
import { ChartIcon, ClockIcon, FolderIcon, ListIcon, SearchIcon, UsersIcon } from "./components/Icons";

type Tab = {
  label: string;
  icon: () => JSX.Element;
  active: () => boolean;
  go: () => void;
};

function timeTabs(): Tab[] {
  const list: Tab[] = [
    {
      label: t("nav.today"),
      icon: () => <ClockIcon class="h-6 w-6" />,
      active: () => view() === "today",
      go: () => setView("today"),
    },
    {
      label: t("nav.history"),
      icon: () => <ListIcon class="h-6 w-6" />,
      active: () => view() === "history",
      go: () => setView("history"),
    },
    {
      label: t("nav.stats"),
      icon: () => <ChartIcon class="h-6 w-6" />,
      active: () => view() === "stats",
      go: () => setView("stats"),
    },
  ];
  if (roleAtLeast("office")) {
    list.push({
      label: t("nav.team"),
      icon: () => <UsersIcon class="h-6 w-6" />,
      active: () => view() === "team",
      go: () => setView("team"),
    });
    list.push({
      label: t("nav.search"),
      icon: () => <SearchIcon class="h-6 w-6" />,
      active: () => view() === "search",
      go: () => setView("search"),
    });
  }
  return list;
}

function taskTabs(): Tab[] {
  return [
    {
      label: t("nav.projects"),
      icon: () => <FolderIcon class="h-6 w-6" />,
      active: () => taskView() === "projects",
      go: () => goTaskView("projects"),
    },
    {
      label: t("nav.tasks"),
      icon: () => <ListIcon class="h-6 w-6" />,
      active: () => taskView() === "tasks",
      go: () => goTaskView("tasks"),
    },
    {
      label: t("nav.search"),
      icon: () => <SearchIcon class="h-6 w-6" />,
      active: () => taskView() === "search",
      go: () => goTaskView("search"),
    },
    {
      label: t("nav.stats"),
      icon: () => <ChartIcon class="h-6 w-6" />,
      active: () => taskView() === "stats",
      go: () => goTaskView("stats"),
    },
  ];
}

function WorldSwitcher(props: { fill?: boolean }) {
  const base = "rounded-md text-xs font-semibold uppercase tracking-wide transition-colors";
  const size = () => (props.fill ? "flex-1 px-3 py-2.5" : "px-3 py-1.5");
  return (
    <div class={`flex rounded-lg border border-line bg-background p-0.5 ${props.fill ? "w-full" : ""}`}>
      <button
        type="button"
        onClick={() => switchWorld("time")}
        class={`${base} ${size()} ${
          world() === "time" ? "bg-primary text-primary-foreground" : "text-muted hover:text-foreground"
        }`}
      >
        {t("world.time")}
      </button>
      <button
        type="button"
        onClick={() => switchWorld("tasks")}
        class={`${base} ${size()} ${
          world() === "tasks" ? "bg-primary text-primary-foreground" : "text-muted hover:text-foreground"
        }`}
      >
        {t("world.tasks")}
      </button>
    </div>
  );
}

function TabBar(props: { tabs: Tab[] }) {
  return (
    <nav class="overflow-x-auto border-t border-line/60">
      <div class="mx-auto flex max-w-4xl items-center gap-1 px-3 py-1.5">
        <For each={props.tabs}>
          {(tab) => (
            <button
              type="button"
              onClick={tab.go}
              class={`shrink-0 whitespace-nowrap rounded-md px-3 py-1.5 text-[11px] font-semibold uppercase tracking-wide transition-colors ${
                tab.active()
                  ? "bg-soft text-primary"
                  : "text-muted hover:bg-soft/60 hover:text-foreground"
              }`}
            >
              {tab.label}
            </button>
          )}
        </For>
      </div>
    </nav>
  );
}

/**
 * Phone navigation. Pinned to the bottom of the screen so the tabs sit under
 * the thumb, with a 64 px target per item (Material's minimum is 48 px).
 */
function BottomNav(props: { tabs: Tab[] }) {
  return (
    <nav
      aria-label={t("nav.label")}
      class="fixed inset-x-0 bottom-0 z-30 border-t border-line bg-card/95 backdrop-blur sm:hidden"
    >
      <div class="mx-auto flex max-w-4xl items-stretch">
        <For each={props.tabs}>
          {(tab) => (
            <button
              type="button"
              onClick={tab.go}
              aria-current={tab.active() ? "page" : undefined}
              class={`relative flex min-h-16 flex-1 flex-col items-center justify-center gap-1 px-1 py-2 transition-colors ${
                tab.active() ? "text-primary" : "text-muted active:bg-soft"
              }`}
            >
              <span
                class={`absolute inset-x-4 top-0 h-0.5 rounded-full ${
                  tab.active() ? "bg-primary" : "bg-transparent"
                }`}
              />
              {tab.icon()}
              <span class="text-[11px] font-semibold uppercase leading-none tracking-wide">
                {tab.label}
              </span>
            </button>
          )}
        </For>
      </div>
    </nav>
  );
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
  const inGlobal = () => globalView() !== null;

  return (
    <div class="flex min-h-screen flex-col">
      <header class="sticky top-0 z-30 border-b border-line bg-card/95 backdrop-blur">
        <div class="mx-auto flex max-w-4xl items-center justify-between gap-2 px-4 py-2">
          <div class="min-w-0">
            <Show
              when={org()?.logo_path}
              fallback={
                <div class="truncate font-display text-lg font-bold uppercase leading-none tracking-wide">
                  {org()?.name ?? "Daywerk"}
                </div>
              }
            >
              {(src) => (
                <img
                  src={assetUrl(src()) ?? ""}
                  alt={org()?.name ?? "Daywerk"}
                  class="h-7 w-auto max-w-[9rem] object-contain sm:max-w-[14rem]"
                />
              )}
            </Show>
            <div class="truncate text-xs text-muted">
              {user()?.name}
              <Show when={user()?.role !== "employee"}>
                <span class="ml-1 rounded bg-soft px-1.5 py-0.5 text-[10px] font-semibold uppercase text-primary">
                  {t(`admin.role.${user()?.role}` as "admin.role.employee")}
                </span>
              </Show>
            </div>
          </div>

          <div class="hidden items-center gap-2 sm:flex">
            <WorldSwitcher />
          </div>

          <div class="flex items-center gap-1">
            <button
              type="button"
              onClick={openSettings}
              title={t("settings.title")}
              class={`rounded-lg p-2.5 hover:bg-soft ${
                globalView() === "settings" ? "text-primary" : "text-muted hover:text-foreground"
              }`}
            >
              <CogIcon class="h-5 w-5" />
            </button>
            <button
              type="button"
              onClick={() => void signOut()}
              title={t("settings.logout")}
              class="rounded-lg p-2.5 text-muted hover:bg-soft hover:text-foreground"
            >
              <LogoutIcon class="h-5 w-5" />
            </button>
          </div>
        </div>

        <div class="px-4 pb-2 sm:hidden">
          <WorldSwitcher fill />
        </div>

        <Show when={!inGlobal()}>
          <div class="hidden sm:block">
            <TabBar tabs={world() === "time" ? timeTabs() : taskTabs()} />
          </div>
        </Show>
      </header>

      <main
        class={`mx-auto w-full max-w-4xl flex-1 px-4 pt-4 ${
          inGlobal() ? "pb-4" : "pb-24 sm:pb-4"
        }`}
      >
        <Show when={inGlobal()}>
          <button
            type="button"
            class="mb-3 inline-flex items-center gap-1 text-xs font-semibold uppercase tracking-wide text-muted hover:text-foreground"
            onClick={() => setGlobalView(globalView() === "admin" ? "settings" : null)}
          >
            ← {t("settings.back")}
          </button>
        </Show>

        <Switch>
          <Match when={globalView() === "settings"}>
            <Settings />
          </Match>
          <Match when={globalView() === "admin"}>
            <Admin />
          </Match>
          <Match when={globalView() === null && world() === "time"}>
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
              <Match when={view() === "search"}>
                <Search />
              </Match>
            </Switch>
          </Match>
          <Match when={globalView() === null && world() === "tasks"}>
            <TaskWorld />
          </Match>
        </Switch>
      </main>

      <Show when={!inGlobal()}>
        <BottomNav tabs={world() === "time" ? timeTabs() : taskTabs()} />
      </Show>
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
