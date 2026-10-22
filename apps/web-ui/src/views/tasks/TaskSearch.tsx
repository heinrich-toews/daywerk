import { For, Show, createEffect, createSignal, onCleanup } from "solid-js";
import { api, type TaskRow } from "../../api";
import TaskRowItem from "../../components/TaskRowItem";
import { t } from "../../i18n";
import { openTask } from "../../state";
import { PRIORITIES, STATUSES, priorityLabel, statusLabel } from "./util";

export default function TaskSearch() {
  const [q, setQ] = createSignal("");
  const [debounced, setDebounced] = createSignal("");
  const [status, setStatus] = createSignal("");
  const [priority, setPriority] = createSignal("");
  const [rows, setRows] = createSignal<TaskRow[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [searched, setSearched] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  createEffect(() => {
    const value = q();
    const handle = window.setTimeout(() => setDebounced(value), 300);
    onCleanup(() => window.clearTimeout(handle));
  });

  createEffect(() => {
    const term = debounced().trim();
    const state = status();
    const level = priority();
    if (!term && !state && !level) {
      setRows([]);
      setSearched(false);
      return;
    }
    setLoading(true);
    api
      .tasks({
        q: term || undefined,
        status: state || undefined,
        priority: level || undefined,
        limit: 100,
      })
      .then((data) => {
        setRows(data);
        setSearched(true);
        setError(null);
      })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  });

  return (
    <div class="space-y-4">
      <h1 class="font-display text-2xl font-bold uppercase tracking-wide">{t("nav.search")}</h1>

      <div class="card space-y-2 p-4">
        <input
          type="search"
          class="field"
          placeholder={t("tasks.searchPlaceholder")}
          value={q()}
          onInput={(e) => setQ(e.currentTarget.value)}
        />
        <div class="grid grid-cols-2 gap-2">
          <select
            class="field"
            value={status()}
            onChange={(e) => setStatus(e.currentTarget.value)}
          >
            <option value="">{t("tasks.status")}: {t("tasks.filterAll")}</option>
            <For each={STATUSES}>{(value) => <option value={value}>{statusLabel(value)}</option>}</For>
          </select>
          <select
            class="field"
            value={priority()}
            onChange={(e) => setPriority(e.currentTarget.value)}
          >
            <option value="">{t("tasks.priority")}: {t("tasks.filterAll")}</option>
            <For each={PRIORITIES}>
              {(value) => <option value={value}>{priorityLabel(value)}</option>}
            </For>
          </select>
        </div>
      </div>

      <Show when={error()}>
        {(message) => <p class="rounded-lg bg-danger/10 px-3 py-2 text-sm text-danger">{message()}</p>}
      </Show>

      <Show when={loading()}>
        <p class="text-sm text-muted">{t("common.loading")}</p>
      </Show>

      <Show when={!loading() && !searched()}>
        <p class="card p-6 text-center text-sm text-muted">{t("tasks.searchPlaceholder")}</p>
      </Show>

      <Show when={!loading() && searched()}>
        <Show
          when={rows().length > 0}
          fallback={<p class="card p-6 text-center text-sm text-muted">{t("search.empty")}</p>}
        >
          <div class="space-y-1.5">
            <For each={rows()}>
              {(row) => <TaskRowItem row={row} showProject onOpen={() => openTask(row.id)} />}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}
