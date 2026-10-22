import { For, Show, createSignal, onMount } from "solid-js";
import { api, type Account, type ProjectRow } from "../../api";
import Sheet from "../../components/Sheet";
import { t } from "../../i18n";
import { openProject, roleAtLeast } from "../../state";
import { booked, effort } from "./util";

export default function ProjectsView() {
  const [rows, setRows] = createSignal<ProjectRow[]>([]);
  const [accounts, setAccounts] = createSignal<Account[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [showForm, setShowForm] = createSignal(false);

  const [name, setName] = createSignal("");
  const [description, setDescription] = createSignal("");
  const [accountId, setAccountId] = createSignal("");
  const [busy, setBusy] = createSignal(false);

  async function load() {
    setLoading(true);
    try {
      setRows(await api.projects());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  onMount(() => {
    void load();
    api.accounts().then(setAccounts).catch(() => setAccounts([]));
  });

  async function create(e: Event) {
    e.preventDefault();
    if (!name().trim()) return;
    setBusy(true);
    setError(null);
    try {
      await api.createProject({
        name: name().trim(),
        description: description().trim(),
        account_id: accountId() || null,
      });
      setName("");
      setDescription("");
      setAccountId("");
      setShowForm(false);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class="space-y-4">
      <div class="flex items-center justify-between gap-2">
        <h1 class="font-display text-2xl font-bold uppercase tracking-wide">
          {t("tasks.projects")}
        </h1>
        <Show when={roleAtLeast("office")}>
          <button type="button" class="btn-primary px-3 py-2 text-sm" onClick={() => setShowForm(true)}>
            + {t("tasks.newProject")}
          </button>
        </Show>
      </div>

      <Show when={error()}>
        {(message) => <p class="rounded-lg bg-danger/10 px-3 py-2 text-sm text-danger">{message()}</p>}
      </Show>

      <Show when={!loading()} fallback={<p class="text-sm text-muted">{t("common.loading")}</p>}>
        <Show
          when={rows().length > 0}
          fallback={<p class="card p-6 text-center text-sm text-muted">{t("tasks.noProjects")}</p>}
        >
          <div class="space-y-2">
            <For each={rows()}>
              {(row) => (
                <button
                  type="button"
                  onClick={() => openProject(row.id)}
                  class="card w-full p-4 text-left transition-colors hover:border-primary/40"
                >
                  <div class="flex items-baseline justify-between gap-2">
                    <h2 class="truncate font-display text-lg font-bold uppercase">{row.name}</h2>
                    <Show when={row.status === "archived"}>
                      <span class="rounded bg-line px-1.5 py-0.5 text-[10px] font-semibold uppercase text-muted">
                        {t("admin.archived")}
                      </span>
                    </Show>
                  </div>
                  <Show when={row.description}>
                    <p class="mt-0.5 line-clamp-2 text-xs text-muted">{row.description}</p>
                  </Show>

                  <div class="mt-2 flex flex-wrap gap-x-5 gap-y-1 text-[11px] text-muted">
                    <span>
                      {t("tasks.openTasks")}: <b class="text-foreground">{row.open_count}</b>
                      {" · "}
                      {t("tasks.doneTasks")}: <b class="text-foreground">{row.done_count}</b>
                    </span>
                    <span>
                      {t("tasks.planned")}: <b class="text-foreground">{effort(row.estimate_minutes)}</b>
                      {" · "}
                      {t("tasks.actual")}: <b class="text-foreground">{effort(row.spent_minutes)}</b>
                    </span>
                    <Show when={row.booked_ms > 0}>
                      <span>
                        {t("tasks.booked")}: <b class="text-primary">{booked(row.booked_ms)}</b>
                      </span>
                    </Show>
                    <Show when={row.overdue_count > 0}>
                      <span class="font-semibold text-danger">
                        {row.overdue_count} {t("tasks.overdue")}
                      </span>
                    </Show>
                  </div>

                  <Show when={row.task_count > 0}>
                    <div class="mt-2 h-1.5 rounded-full bg-soft">
                      <div
                        class="h-1.5 rounded-full bg-primary"
                        style={{ width: `${Math.round((row.done_count / row.task_count) * 100)}%` }}
                      />
                    </div>
                  </Show>
                </button>
              )}
            </For>
          </div>
        </Show>
      </Show>

      <Sheet open={showForm()} title={t("tasks.newProject")} onClose={() => setShowForm(false)}>
        <form onSubmit={create} class="space-y-2">
          <div>
            <label class="label" for="project-name">{t("tasks.project")}</label>
            <input
              id="project-name"
              class="field"
              required
              value={name()}
              onInput={(e) => setName(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="label" for="project-desc">{t("tasks.description")}</label>
            <textarea
              id="project-desc"
              rows={2}
              class="field resize-none"
              value={description()}
              onInput={(e) => setDescription(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="label" for="project-account">{t("tasks.account")}</label>
            <select
              id="project-account"
              class="field"
              value={accountId()}
              onChange={(e) => setAccountId(e.currentTarget.value)}
            >
              <option value="">{t("tasks.noAccount")}</option>
              <For each={accounts().filter((a) => a.kind === "work")}>
                {(account) => <option value={account.id}>{account.name}</option>}
              </For>
            </select>
          </div>
          <div class="flex gap-2 pt-1">
            <button type="button" class="btn-ghost flex-1" onClick={() => setShowForm(false)}>
              {t("common.close")}
            </button>
            <button type="submit" class="btn-primary flex-1" disabled={busy()}>
              {busy() ? t("common.loading") : t("common.add")}
            </button>
          </div>
        </form>
      </Sheet>
    </div>
  );
}
