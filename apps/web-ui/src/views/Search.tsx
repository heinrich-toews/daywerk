import { For, Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { api, type BookingRow, type Org, type User } from "../api";
import { dur, monthStartIso, timeOfDay, todayIso } from "../format";
import { locale, t } from "../i18n";
import { roleAtLeast } from "../state";

export default function Search() {
  const [q, setQ] = createSignal("");
  const [debounced, setDebounced] = createSignal("");
  const [from, setFrom] = createSignal(monthStartIso());
  const [to, setTo] = createSignal(todayIso());
  const [userId, setUserId] = createSignal("");
  const [orgId, setOrgId] = createSignal("");
  const [users, setUsers] = createSignal<User[]>([]);
  const [orgs, setOrgs] = createSignal<Org[]>([]);
  const [rows, setRows] = createSignal<BookingRow[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [searched, setSearched] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  onMount(() => {
    api.users().then(setUsers).catch(() => setUsers([]));
    if (roleAtLeast("admin")) {
      api.orgs().then(setOrgs).catch(() => setOrgs([]));
    }
  });

  // debounce typing so a search does not fire on every keystroke
  createEffect(() => {
    const value = q();
    const handle = window.setTimeout(() => setDebounced(value), 300);
    onCleanup(() => window.clearTimeout(handle));
  });

  createEffect(() => {
    const term = debounced().trim();
    const f = from();
    const until = to();
    const user = userId();
    const org = orgId();

    if (!term) {
      setRows([]);
      setSearched(false);
      return;
    }

    setLoading(true);
    api
      .bookings({
        q: term,
        from: f,
        to: until,
        user_id: user || undefined,
        org_id: org || undefined,
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
      <h1 class="font-display text-2xl font-bold uppercase tracking-wide">
        {t("search.title")}
      </h1>

      <div class="card space-y-3 p-4">
        <div>
          <label class="label" for="search-q">
            {t("search.title")}
          </label>
          <input
            id="search-q"
            type="search"
            class="field"
            placeholder={t("search.placeholder")}
            value={q()}
            onInput={(e) => setQ(e.currentTarget.value)}
          />
        </div>

        <div class="grid grid-cols-2 gap-2">
          <div>
            <label class="label" for="search-from">
              {t("history.from")}
            </label>
            <input
              id="search-from"
              type="date"
              class="field"
              value={from()}
              onInput={(e) => setFrom(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="label" for="search-to">
              {t("history.to")}
            </label>
            <input
              id="search-to"
              type="date"
              class="field"
              value={to()}
              onInput={(e) => setTo(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="label" for="search-user">
              {t("search.worker")}
            </label>
            <select
              id="search-user"
              class="field"
              value={userId()}
              onChange={(e) => setUserId(e.currentTarget.value)}
            >
              <option value="">{t("search.allWorkers")}</option>
              <For each={users()}>{(u) => <option value={u.id}>{u.name}</option>}</For>
            </select>
          </div>
          <Show when={roleAtLeast("admin")}>
            <div>
              <label class="label" for="search-org">
                {t("team.org")}
              </label>
              <select
                id="search-org"
                class="field"
                value={orgId()}
                onChange={(e) => setOrgId(e.currentTarget.value)}
              >
                <option value="">{t("team.allOrgs")}</option>
                <For each={orgs()}>{(o) => <option value={o.id}>{o.name}</option>}</For>
              </select>
            </div>
          </Show>
        </div>
      </div>

      <Show when={error()}>
        {(message) => (
          <p class="rounded-lg bg-danger/10 px-3 py-2 text-sm text-danger">{message()}</p>
        )}
      </Show>

      <Show when={loading()}>
        <p class="text-sm text-muted">{t("common.loading")}</p>
      </Show>

      <Show when={!loading() && !q().trim()}>
        <p class="card p-6 text-center text-sm text-muted">{t("search.hint")}</p>
      </Show>

      <Show when={!loading() && q().trim() && searched()}>
        <p class="text-[11px] font-semibold uppercase tracking-wide text-muted">
          {t("search.results", { n: rows().length })}
        </p>
        <Show
          when={rows().length > 0}
          fallback={<p class="card p-6 text-center text-sm text-muted">{t("search.empty")}</p>}
        >
          <div class="space-y-2">
            <For each={rows()}>
              {(row) => (
                <article class="card p-3">
                  <header class="flex flex-wrap items-baseline justify-between gap-2">
                    <div class="min-w-0">
                      <span class="font-display text-base font-bold uppercase">
                        {row.user_name}
                      </span>
                      <span class="ml-2 text-xs text-muted">
                        {row.date} · {row.org_name}
                      </span>
                    </div>
                    <span class="text-sm tabular-nums text-muted">
                      {timeOfDay(row.started_at, locale())}–
                      {row.ended_at ? timeOfDay(row.ended_at, locale()) : "…"}
                      <b class="ml-2 text-foreground">
                        {dur((row.ended_at ?? row.started_at) - row.started_at)}
                      </b>
                    </span>
                  </header>
                  <div class="mt-1 text-sm font-medium">{row.account_name}</div>
                  <Show when={row.note}>
                    <p class="mt-1 text-xs text-muted">
                      <span class="font-semibold uppercase">{t("note.note")}:</span> {row.note}
                    </p>
                  </Show>
                  <Show when={row.material}>
                    <p class="text-xs text-muted">
                      <span class="font-semibold uppercase">{t("note.material")}:</span>{" "}
                      {row.material}
                    </p>
                  </Show>
                </article>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}
