import { For, Show, createEffect, createSignal, onMount } from "solid-js";
import { api, downloadTimesheet, type Org, type TeamRow } from "../api";
import { balanceClass, dur, monthStartIso, todayIso } from "../format";
import { t } from "../i18n";
import { roleAtLeast } from "../state";

export default function Team() {
  const [from, setFrom] = createSignal(monthStartIso());
  const [to, setTo] = createSignal(todayIso());
  const [orgId, setOrgId] = createSignal("");
  const [orgs, setOrgs] = createSignal<Org[]>([]);
  const [rows, setRows] = createSignal<TeamRow[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [exporting, setExporting] = createSignal<"xlsx" | "pdf" | null>(null);
  const [exportError, setExportError] = createSignal<string | null>(null);

  onMount(() => {
    if (roleAtLeast("admin")) {
      api.orgs().then(setOrgs).catch(() => setOrgs([]));
    }
  });

  createEffect(() => {
    const f = from();
    const until = to();
    const org = orgId();
    setLoading(true);
    api
      .team(f, until, org || undefined)
      .then((data) => {
        setRows(data);
        setError(null);
      })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  });

  async function download(kind: "xlsx" | "pdf") {
    setExporting(kind);
    setExportError(null);
    try {
      await downloadTimesheet(kind, {
        from: from(),
        to: to(),
        org_id: orgId() || undefined,
      });
    } catch (e) {
      setExportError(`${t("export.failed")} ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setExporting(null);
    }
  }

  return (
    <div class="space-y-4">
      <h1 class="font-display text-2xl font-bold uppercase tracking-wide">{t("team.title")}</h1>

      <div class="card flex flex-wrap items-end gap-3 p-4">
        <div class="flex-1">
          <label class="label" for="team-from">
            {t("history.from")}
          </label>
          <input
            id="team-from"
            type="date"
            class="field"
            value={from()}
            onInput={(e) => setFrom(e.currentTarget.value)}
          />
        </div>
        <div class="flex-1">
          <label class="label" for="team-to">
            {t("history.to")}
          </label>
          <input
            id="team-to"
            type="date"
            class="field"
            value={to()}
            onInput={(e) => setTo(e.currentTarget.value)}
          />
        </div>
        <Show when={roleAtLeast("admin")}>
          <div class="min-w-48 flex-1">
            <label class="label" for="team-org">
              {t("team.org")}
            </label>
            <select
              id="team-org"
              class="field"
              value={orgId()}
              onChange={(e) => setOrgId(e.currentTarget.value)}
            >
              <option value="">{t("team.allOrgs")}</option>
              <For each={orgs()}>{(org) => <option value={org.id}>{org.name}</option>}</For>
            </select>
          </div>
        </Show>
      </div>

      <div class="flex gap-2">
        <button
          type="button"
          class="btn-ghost flex-1"
          disabled={!!exporting()}
          onClick={() => void download("xlsx")}
        >
          {exporting() === "xlsx" ? t("common.loading") : `${t("export.title")} XLSX`}
        </button>
        <button
          type="button"
          class="btn-ghost flex-1"
          disabled={!!exporting()}
          onClick={() => void download("pdf")}
        >
          {exporting() === "pdf" ? t("common.loading") : `${t("export.title")} PDF`}
        </button>
      </div>

      <Show when={exportError()}>
        {(message) => <p class="text-sm font-medium text-danger">{message()}</p>}
      </Show>

      <Show when={error()}>
        {(message) => (
          <p class="rounded-lg bg-danger/10 px-3 py-2 text-sm text-danger">{message()}</p>
        )}
      </Show>

      <Show when={!loading()} fallback={<p class="text-sm text-muted">{t("common.loading")}</p>}>
        <Show
          when={rows().length > 0}
          fallback={<p class="card p-6 text-center text-sm text-muted">{t("team.empty")}</p>}
        >
          <div class="space-y-2">
            <For each={rows()}>
              {(row) => (
                <article class="card flex flex-wrap items-center justify-between gap-3 p-4">
                  <div class="min-w-0">
                    <div class="truncate font-display text-lg font-bold uppercase leading-tight">
                      {row.user.name}
                      <Show when={!row.user.active}>
                        <span class="ml-2 rounded bg-line px-1.5 py-0.5 align-middle text-[10px] font-semibold uppercase text-muted">
                          {t("admin.inactive")}
                        </span>
                      </Show>
                    </div>
                    <div class="truncate text-xs text-muted">
                      {row.org_name}
                      <Show when={row.user.daily_target_minutes !== null}>
                        <span class="ml-2">
                          {t("admin.targetUser")}: {row.user.daily_target_minutes}
                        </span>
                      </Show>
                    </div>
                  </div>

                  <div class="flex gap-4 text-right">
                    <div>
                      <div class="text-[10px] font-semibold uppercase text-muted">
                        {t("team.worked")}
                      </div>
                      <div class="font-display text-lg font-bold tabular-nums">
                        {dur(row.period.worked_ms)}
                      </div>
                    </div>
                    <div>
                      <div class="text-[10px] font-semibold uppercase text-muted">
                        {t("team.balancePeriod")}
                      </div>
                      <div
                        class={`font-display text-lg font-bold tabular-nums ${balanceClass(
                          row.period.balance_ms,
                        )}`}
                      >
                        {dur(row.period.balance_ms)}
                      </div>
                    </div>
                    <div>
                      <div class="text-[10px] font-semibold uppercase text-muted">
                        {t("team.balanceTotal")}
                      </div>
                      <div
                        class={`font-display text-lg font-bold tabular-nums ${balanceClass(
                          row.total.balance_ms,
                        )}`}
                      >
                        {dur(row.total.balance_ms)}
                      </div>
                    </div>
                  </div>
                </article>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}
