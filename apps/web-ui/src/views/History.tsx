import { For, Show, createEffect, createSignal } from "solid-js";
import { api, type DayView as DayViewData } from "../api";
import { balanceClass, dur, longDate, monthStartIso, timeOfDay, todayIso } from "../format";
import { locale, t } from "../i18n";
import { me } from "../state";

export default function History() {
  const [from, setFrom] = createSignal(monthStartIso());
  const [to, setTo] = createSignal(todayIso());
  const [items, setItems] = createSignal<DayViewData[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  createEffect(() => {
    const f = from();
    const until = to();
    setLoading(true);
    api
      .days(f, until)
      .then((rows) => {
        setItems(rows);
        setError(null);
      })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  });

  const accountName = (id: string) => me()?.accounts.find((a) => a.id === id)?.name ?? "—";
  const target = () => (me()?.target_minutes ?? 0) * 60_000;

  return (
    <div class="space-y-4">
      <h1 class="font-display text-2xl font-bold uppercase tracking-wide">{t("history.title")}</h1>

      <div class="card flex flex-wrap items-end gap-3 p-4">
        <div class="flex-1">
          <label class="label" for="from">
            {t("history.from")}
          </label>
          <input
            id="from"
            type="date"
            class="field"
            value={from()}
            onInput={(e) => setFrom(e.currentTarget.value)}
          />
        </div>
        <div class="flex-1">
          <label class="label" for="to">
            {t("history.to")}
          </label>
          <input
            id="to"
            type="date"
            class="field"
            value={to()}
            onInput={(e) => setTo(e.currentTarget.value)}
          />
        </div>
      </div>

      <Show when={error()}>
        {(message) => (
          <p class="rounded-lg bg-danger/10 px-3 py-2 text-sm text-danger">{message()}</p>
        )}
      </Show>

      <Show when={!loading()} fallback={<p class="text-sm text-muted">{t("common.loading")}</p>}>
        <Show
          when={items().length > 0}
          fallback={
            <p class="card p-6 text-center text-sm text-muted">{t("history.empty")}</p>
          }
        >
          <div class="space-y-3">
            <For each={items()}>
              {(entry) => (
                <article class="card p-4">
                  <header class="flex flex-wrap items-baseline justify-between gap-2">
                    <h2 class="font-display text-lg font-bold uppercase">
                      {longDate(entry.day.date, locale())}
                    </h2>
                    <div class="flex items-center gap-2 text-sm">
                      <span class="chip">{dur(entry.totals.work_ms)}</span>
                      <span
                        class={`text-xs font-semibold ${balanceClass(
                          entry.totals.work_ms - (entry.segments.length > 0 ? target() : 0),
                        )}`}
                      >
                        {entry.segments.length > 0
                          ? dur(entry.totals.work_ms - target())
                          : t("common.none")}
                      </span>
                      <Show when={entry.day.status === "closed"}>
                        <span class="text-[10px] uppercase text-muted">{t("punch.closed")}</span>
                      </Show>
                    </div>
                  </header>

                  <div class="mt-2 space-y-1">
                    <For each={entry.segments}>
                      {(segment) => (
                        <div class="flex items-baseline justify-between gap-2 rounded-md bg-soft/60 px-2 py-1 text-sm">
                          <span class="min-w-0 truncate font-medium">
                            {accountName(segment.account_id)}
                          </span>
                          <span class="shrink-0 tabular-nums text-muted">
                            {timeOfDay(segment.started_at, locale())}–
                            {segment.ended_at
                              ? timeOfDay(segment.ended_at, locale())
                              : "…"}
                            <span class="ml-2 font-semibold text-foreground">
                              {dur((segment.ended_at ?? segment.started_at) - segment.started_at)}
                            </span>
                          </span>
                        </div>
                      )}
                    </For>
                    <Show when={entry.segments.length === 0}>
                      <p class="text-sm text-muted">{t("common.noData")}</p>
                    </Show>
                  </div>

                  <footer class="mt-2 flex gap-4 text-xs text-muted">
                    <span>
                      {t("day.work")}: <b class="text-foreground">{dur(entry.totals.work_ms)}</b>
                    </span>
                    <span>
                      {t("day.break")}: <b class="text-foreground">{dur(entry.totals.break_ms)}</b>
                    </span>
                    <span>
                      {t("day.span")}: <b class="text-foreground">{dur(entry.totals.span_ms)}</b>
                    </span>
                  </footer>
                </article>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}
