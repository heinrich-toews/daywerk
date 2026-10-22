import { For, Show, createEffect, createSignal } from "solid-js";
import { api, type TaskStats as TaskStatsData } from "../../api";
import { monthStartIso, todayIso } from "../../format";
import { t } from "../../i18n";
import { booked, effort } from "./util";

export default function TaskStats() {
  const [from, setFrom] = createSignal(monthStartIso());
  const [to, setTo] = createSignal(todayIso());
  const [data, setData] = createSignal<TaskStatsData | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  createEffect(() => {
    const start = from();
    const end = to();
    setLoading(true);
    api
      .taskStats(start, end)
      .then((stats) => {
        setData(stats);
        setError(null);
      })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  });

  return (
    <div class="space-y-4">
      <h1 class="font-display text-2xl font-bold uppercase tracking-wide">{t("nav.stats")}</h1>

      <div class="card flex flex-wrap items-end gap-3 p-4">
        <div class="flex-1">
          <label class="label" for="task-stats-from">{t("history.from")}</label>
          <input
            id="task-stats-from"
            type="date"
            class="field"
            value={from()}
            onInput={(e) => setFrom(e.currentTarget.value)}
          />
        </div>
        <div class="flex-1">
          <label class="label" for="task-stats-to">{t("history.to")}</label>
          <input
            id="task-stats-to"
            type="date"
            class="field"
            value={to()}
            onInput={(e) => setTo(e.currentTarget.value)}
          />
        </div>
      </div>

      <Show when={error()}>
        {(message) => <p class="rounded-lg bg-danger/10 px-3 py-2 text-sm text-danger">{message()}</p>}
      </Show>

      <Show when={!loading() && data()} fallback={<p class="text-sm text-muted">{t("common.loading")}</p>}>
        {(stats) => (
          <>
            <section class="grid grid-cols-2 gap-2 sm:grid-cols-4">
              <div class="card p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">{t("nav.tasks")}</div>
                <div class="font-display text-xl font-bold tabular-nums">{stats().total_tasks}</div>
              </div>
              <div class="card p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">{t("tasks.openTasks")}</div>
                <div class="font-display text-xl font-bold tabular-nums">{stats().open_tasks}</div>
              </div>
              <div class="card p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">{t("tasks.overdue")}</div>
                <div class="font-display text-xl font-bold tabular-nums text-danger">
                  {stats().overdue_tasks}
                </div>
              </div>
              <div class="card p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">{t("tasks.doneTasks")}</div>
                <div class="font-display text-xl font-bold tabular-nums text-ok">
                  {stats().done_tasks}
                </div>
              </div>
              <div class="card p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">{t("tasks.planned")}</div>
                <div class="font-display text-xl font-bold tabular-nums">
                  {effort(stats().estimate_minutes)}
                </div>
              </div>
              <div class="card p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">{t("tasks.actual")}</div>
                <div class="font-display text-xl font-bold tabular-nums">
                  {effort(stats().spent_minutes)}
                </div>
              </div>
              <div class="card col-span-2 p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">{t("tasks.booked")}</div>
                <div class="font-display text-xl font-bold tabular-nums text-primary">
                  {booked(stats().booked_ms)}
                </div>
              </div>
            </section>

            <StatsTable title={t("tasks.byProject")} rows={stats().by_project} />
            <StatsTable title={t("tasks.byAssignee")} rows={stats().by_assignee} />
          </>
        )}
      </Show>
    </div>
  );
}

function StatsTable(props: {
  title: string;
  rows: TaskStatsData["by_project"];
}) {
  return (
    <section class="card p-4">
      <h2 class="mb-2 font-display text-lg font-bold uppercase">{props.title}</h2>
      <Show
        when={props.rows.length > 0}
        fallback={<p class="text-sm text-muted">{t("common.noData")}</p>}
      >
        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b border-line text-left text-[10px] uppercase tracking-wide text-muted">
                <th class="py-1 pr-2 font-semibold">{t("tasks.project")}</th>
                <th class="px-2 py-1 text-right font-semibold">{t("nav.tasks")}</th>
                <th class="px-2 py-1 text-right font-semibold">{t("tasks.doneTasks")}</th>
                <th class="px-2 py-1 text-right font-semibold">{t("tasks.planned")}</th>
                <th class="px-2 py-1 text-right font-semibold">{t("tasks.actual")}</th>
                <th class="py-1 pl-2 text-right font-semibold">{t("tasks.booked")}</th>
              </tr>
            </thead>
            <tbody>
              <For each={props.rows}>
                {(row) => (
                  <tr class="border-b border-line/60 last:border-0">
                    <td class="max-w-40 truncate py-1 pr-2">{row.name}</td>
                    <td class="px-2 py-1 text-right tabular-nums">{row.task_count}</td>
                    <td class="px-2 py-1 text-right tabular-nums">{row.done_count}</td>
                    <td class="px-2 py-1 text-right tabular-nums">{effort(row.estimate_minutes)}</td>
                    <td class="px-2 py-1 text-right tabular-nums">{effort(row.spent_minutes)}</td>
                    <td class="py-1 pl-2 text-right tabular-nums text-primary">
                      {row.booked_ms > 0 ? booked(row.booked_ms) : "–"}
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>
      </Show>
    </section>
  );
}
