import { For, Show, createSignal, onMount } from "solid-js";
import { api, type TaskRow } from "../../api";
import TaskRowItem from "../../components/TaskRowItem";
import { t } from "../../i18n";
import { openTask } from "../../state";
import { STATUSES, statusLabel } from "./util";

export default function TaskListView() {
  const [rows, setRows] = createSignal<TaskRow[]>([]);
  const [mine, setMine] = createSignal(true);
  const [status, setStatus] = createSignal("");
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  async function load() {
    setLoading(true);
    try {
      setRows(await api.tasks({ mine: mine(), status: status() || undefined }));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  onMount(() => {
    void load();
  });

  function toggle(next: boolean) {
    setMine(next);
    void load();
  }

  function pickStatus(next: string) {
    setStatus(next);
    void load();
  }

  return (
    <div class="space-y-4">
      <h1 class="font-display text-2xl font-bold uppercase tracking-wide">{t("nav.tasks")}</h1>

      <div class="flex flex-wrap items-center gap-2">
        <div class="flex rounded-lg border border-line p-0.5">
          <button
            type="button"
            class={`rounded-md px-3 py-1.5 text-xs font-semibold uppercase ${
              mine() ? "bg-primary text-primary-foreground" : "text-muted"
            }`}
            onClick={() => toggle(true)}
          >
            {t("tasks.filterMine")}
          </button>
          <button
            type="button"
            class={`rounded-md px-3 py-1.5 text-xs font-semibold uppercase ${
              !mine() ? "bg-primary text-primary-foreground" : "text-muted"
            }`}
            onClick={() => toggle(false)}
          >
            {t("tasks.filterAll")}
          </button>
        </div>

        <select
          class="rounded-lg border border-line bg-card px-2 py-1.5 text-sm"
          value={status()}
          onChange={(e) => pickStatus(e.currentTarget.value)}
        >
          <option value="">{t("tasks.status")}: {t("tasks.filterAll")}</option>
          <For each={STATUSES}>{(value) => <option value={value}>{statusLabel(value)}</option>}</For>
        </select>
      </div>

      <Show when={error()}>
        {(message) => <p class="rounded-lg bg-danger/10 px-3 py-2 text-sm text-danger">{message()}</p>}
      </Show>

      <Show when={!loading()} fallback={<p class="text-sm text-muted">{t("common.loading")}</p>}>
        <Show
          when={rows().length > 0}
          fallback={<p class="card p-6 text-center text-sm text-muted">{t("tasks.noTasks")}</p>}
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
