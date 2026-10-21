import { For, Show, createEffect, createSignal } from "solid-js";
import { api, type Stats as StatsData } from "../api";
import { balanceClass, dur, monthStartIso, todayIso } from "../format";
import { t } from "../i18n";

export default function Stats() {
  const [from, setFrom] = createSignal(monthStartIso());
  const [to, setTo] = createSignal(todayIso());
  const [data, setData] = createSignal<StatsData | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  createEffect(() => {
    const f = from();
    const until = to();
    setLoading(true);
    api
      .stats(f, until)
      .then((stats) => {
        setData(stats);
        setError(null);
      })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  });

  const maxAccount = () => Math.max(1, ...(data()?.per_account.map((a) => a.ms) ?? [1]));

  return (
    <div class="space-y-4">
      <h1 class="font-display text-2xl font-bold uppercase tracking-wide">{t("stats.title")}</h1>

      <div class="card flex flex-wrap items-end gap-3 p-4">
        <div class="flex-1">
          <label class="label" for="stats-from">
            {t("history.from")}
          </label>
          <input
            id="stats-from"
            type="date"
            class="field"
            value={from()}
            onInput={(e) => setFrom(e.currentTarget.value)}
          />
        </div>
        <div class="flex-1">
          <label class="label" for="stats-to">
            {t("history.to")}
          </label>
          <input
            id="stats-to"
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

      <Show when={!loading() && data()} fallback={<p class="text-sm text-muted">{t("common.loading")}</p>}>
        {(stats) => (
          <>
            <section class="grid grid-cols-2 gap-2 sm:grid-cols-4">
              <div class="card p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">{t("stats.worked")}</div>
                <div class="font-display text-xl font-bold tabular-nums">
                  {dur(stats().period.worked_ms)}
                </div>
              </div>
              <div class="card p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">{t("stats.days")}</div>
                <div class="font-display text-xl font-bold tabular-nums">{stats().period.days}</div>
              </div>
              <div class="card p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">{t("day.target")}</div>
                <div class="font-display text-xl font-bold tabular-nums">
                  {dur(stats().period.target_ms)}
                </div>
              </div>
              <div class="card p-3 text-center">
                <div class="text-[10px] font-semibold uppercase text-muted">
                  {t("balance.month")}
                </div>
                <div
                  class={`font-display text-xl font-bold tabular-nums ${balanceClass(
                    stats().period.balance_ms,
                  )}`}
                >
                  {dur(stats().period.balance_ms)}
                </div>
              </div>
            </section>

            <section class="card p-4">
              <h2 class="mb-3 font-display text-lg font-bold uppercase">
                {t("stats.perAccount")}
              </h2>
              <div class="space-y-2">
                <For each={stats().per_account}>
                  {(row) => (
                    <div>
                      <div class="flex justify-between text-sm">
                        <span class="font-medium">{row.account_name}</span>
                        <span class="tabular-nums text-muted">{dur(row.ms)}</span>
                      </div>
                      <div class="mt-1 h-2 rounded-full bg-soft">
                        <div
                          class={`h-2 rounded-full ${
                            row.kind === "break" ? "bg-amber-400" : "bg-primary"
                          }`}
                          style={{ width: `${Math.round((row.ms / maxAccount()) * 100)}%` }}
                        />
                      </div>
                    </div>
                  )}
                </For>
                <Show when={stats().per_account.length === 0}>
                  <p class="text-sm text-muted">{t("common.noData")}</p>
                </Show>
              </div>
            </section>

            <section class="card p-4">
              <h2 class="mb-3 font-display text-lg font-bold uppercase">{t("stats.perDay")}</h2>
              <div class="divide-y divide-line">
                <For each={stats().per_day}>
                  {(day) => (
                    <div class="flex items-center justify-between py-1.5 text-sm">
                      <span class="tabular-nums">{day.date}</span>
                      <span class="flex gap-3 tabular-nums">
                        <span class="text-muted">{t("day.break")} {dur(day.break_ms)}</span>
                        <b>{dur(day.work_ms)}</b>
                      </span>
                    </div>
                  )}
                </For>
                <Show when={stats().per_day.length === 0}>
                  <p class="py-2 text-sm text-muted">{t("history.empty")}</p>
                </Show>
              </div>
            </section>

            <p class="text-center text-xs text-muted">
              {t("balance.total")}:{" "}
              <b class={balanceClass(stats().total.balance_ms)}>{dur(stats().total.balance_ms)}</b>
            </p>
          </>
        )}
      </Show>
    </div>
  );
}
