import { For, Show, createSignal, onMount } from "solid-js";
import { api, type Member, type ProjectDetail as ProjectDetailData, type TaskRow } from "../../api";
import Sheet from "../../components/Sheet";
import TaskFormSheet from "../../components/TaskFormSheet";
import TaskRowItem from "../../components/TaskRowItem";
import { t } from "../../i18n";
import { closeProject, openTask, openProjectId, roleAtLeast } from "../../state";
import { booked, effort } from "./util";

export default function ProjectDetail() {
  const projectId = openProjectId;
  const [data, setData] = createSignal<ProjectDetailData | null>(null);
  const [members, setMembers] = createSignal<Member[]>([]);
  const [error, setError] = createSignal<string | null>(null);
  const [showTaskForm, setShowTaskForm] = createSignal(false);
  const [showEdit, setShowEdit] = createSignal(false);
  const [showDelete, setShowDelete] = createSignal(false);
  const [preview, setPreview] = createSignal<{
    tasks: number;
    subtasks: number;
    comments: number;
    minutes: number;
    account_name: string | null;
  } | null>(null);

  async function load() {
    const id = projectId();
    if (!id) return;
    try {
      setData(await api.project(id));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  onMount(() => {
    void load();
    api.members().then(setMembers).catch(() => setMembers([]));
  });

  const groups = () => {
    const tasks = data()?.tasks ?? [];
    return [
      { status: "open", rows: tasks.filter((task) => task.status === "open") },
      { status: "in_progress", rows: tasks.filter((task) => task.status === "in_progress") },
      { status: "done", rows: tasks.filter((task) => task.status === "done") },
      { status: "cancelled", rows: tasks.filter((task) => task.status === "cancelled") },
    ].filter((group) => group.rows.length > 0);
  };

  async function openDelete() {
    const id = projectId();
    if (!id) return;
    try {
      setPreview(await api.projectDeletePreview(id));
      setShowDelete(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function remove() {
    const id = projectId();
    if (!id) return;
    try {
      await api.deleteProject(id);
      setShowDelete(false);
      closeProject();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function toggleArchive() {
    const project = data()?.project;
    if (!project) return;
    try {
      await api.updateProject(project.id, {
        status: project.status === "archived" ? "active" : "archived",
      });
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div class="space-y-4">
      <button
        type="button"
        class="text-xs font-semibold uppercase tracking-wide text-muted hover:text-foreground"
        onClick={closeProject}
      >
        ← {t("tasks.projects")}
      </button>

      <Show when={error()}>
        {(message) => <p class="rounded-lg bg-danger/10 px-3 py-2 text-sm text-danger">{message()}</p>}
      </Show>

      <Show when={data()} fallback={<p class="text-sm text-muted">{t("common.loading")}</p>}>
        {(detail) => (
          <>
            <section class="card p-4">
              <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                  <h1 class="truncate font-display text-2xl font-bold uppercase">
                    {detail().project.name}
                  </h1>
                  <Show when={detail().project.description}>
                    <p class="mt-1 text-sm text-muted">{detail().project.description}</p>
                  </Show>
                </div>
                <Show when={roleAtLeast("office")}>
                  <div class="flex shrink-0 gap-1">
                    <button type="button" class="btn-ghost px-2 py-1 text-xs" onClick={() => setShowEdit(true)}>
                      {t("tasks.edit")}
                    </button>
                    <button type="button" class="btn-ghost px-2 py-1 text-xs" onClick={() => void toggleArchive()}>
                      {detail().project.status === "archived"
                        ? t("tasks.restoreProject")
                        : t("tasks.archiveProject")}
                    </button>
                    <button
                      type="button"
                      class="rounded-lg border border-danger/40 px-2 py-1 text-xs font-semibold text-danger hover:bg-danger/10"
                      onClick={() => void openDelete()}
                    >
                      {t("tasks.delete")}
                    </button>
                  </div>
                </Show>
              </div>

              <div class="mt-3 flex flex-wrap gap-x-5 gap-y-1 text-xs text-muted">
                <span>
                  {t("tasks.account")}:{" "}
                  <b class="text-foreground">
                    {detail().project.account_name ?? t("tasks.noAccount")}
                  </b>
                </span>
                <Show when={detail().project.booked_ms > 0}>
                  <span>
                    {t("tasks.booked")}:{" "}
                    <b class="text-primary">{booked(detail().project.booked_ms)}</b>
                  </span>
                </Show>
                <span>
                  {t("tasks.planned")}: <b class="text-foreground">{effort(detail().project.estimate_minutes)}</b>
                </span>
                <span>
                  {t("tasks.actual")}: <b class="text-foreground">{effort(detail().project.spent_minutes)}</b>
                </span>
              </div>
            </section>

            <div class="flex items-center justify-between gap-2">
              <h2 class="text-xs font-semibold uppercase tracking-wide text-muted">
                {t("tasks.tasksCount", { n: detail().project.task_count })}
              </h2>
              <button
                type="button"
                class="btn-primary px-3 py-2 text-sm"
                onClick={() => setShowTaskForm(true)}
              >
                + {t("tasks.newTask")}
              </button>
            </div>

            <Show
              when={detail().tasks.length > 0}
              fallback={<p class="card p-6 text-center text-sm text-muted">{t("tasks.noTasks")}</p>}
            >
              <div class="space-y-3">
                <For each={groups()}>
                  {(group) => (
                    <section class="space-y-1.5">
                      <h3 class="text-[11px] font-semibold uppercase tracking-wide text-muted">
                        {t(`status.${group.status}` as "status.open")} · {group.rows.length}
                      </h3>
                      <For each={group.rows}>
                        {(row: TaskRow) => <TaskRowItem row={row} onOpen={() => openTask(row.id)} />}
                      </For>
                    </section>
                  )}
                </For>
              </div>
            </Show>

            <TaskFormSheet
              open={showTaskForm()}
              projectId={detail().project.id}
              members={members()}
              onClose={() => setShowTaskForm(false)}
              onSaved={() => void load()}
            />

            <EditProjectSheet
              open={showEdit()}
              detail={detail()}
              onClose={() => setShowEdit(false)}
              onSaved={() => void load()}
            />

            <Sheet
              open={showDelete()}
              title={t("tasks.deleteProject")}
              onClose={() => setShowDelete(false)}
            >
              <div class="space-y-3">
                <p class="text-sm text-muted">{t("tasks.deleteWarning")}</p>
                <Show when={preview()}>
                  {(info) => (
                    <ul class="space-y-1 rounded-lg bg-soft/60 p-3 text-sm">
                      <li>
                        {t("tasks.tasksCount", { n: info().tasks })}
                        <Show when={info().subtasks > 0}> · {info().subtasks} Unteraufgaben</Show>
                      </li>
                      <li>{t("tasks.comments")}: {info().comments}</li>
                      <li>
                        {t("tasks.timeEntries")}: {effort(info().minutes)}
                      </li>
                      <Show when={info().account_name}>
                        <li class="text-muted">
                          {t("tasks.account")}: {info().account_name} →{" "}
                          {t("tasks.noAccount")}
                        </li>
                      </Show>
                    </ul>
                  )}
                </Show>
                <div class="flex gap-2">
                  <button type="button" class="btn-ghost flex-1" onClick={() => setShowDelete(false)}>
                    {t("common.close")}
                  </button>
                  <button
                    type="button"
                    class="flex-1 rounded-lg bg-danger px-3 py-2.5 font-semibold text-white"
                    onClick={() => void remove()}
                  >
                    {t("tasks.confirmDelete")}
                  </button>
                </div>
              </div>
            </Sheet>
          </>
        )}
      </Show>
    </div>
  );
}

function EditProjectSheet(props: {
  open: boolean;
  detail: ProjectDetailData;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [name, setName] = createSignal(props.detail.project.name);
  const [description, setDescription] = createSignal(props.detail.project.description);
  const [accountId, setAccountId] = createSignal(props.detail.project.account_id ?? "");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function save(e: Event) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.updateProject(props.detail.project.id, {
        name: name().trim(),
        description: description().trim(),
        account_id: accountId(),
      });
      props.onSaved();
      props.onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Sheet open={props.open} title={t("tasks.edit")} onClose={props.onClose}>
      <form onSubmit={save} class="space-y-2">
        <div>
          <label class="label" for="edit-name">{t("tasks.project")}</label>
          <input
            id="edit-name"
            class="field"
            required
            value={name()}
            onInput={(e) => setName(e.currentTarget.value)}
          />
        </div>
        <div>
          <label class="label" for="edit-desc">{t("tasks.description")}</label>
          <textarea
            id="edit-desc"
            rows={2}
            class="field resize-none"
            value={description()}
            onInput={(e) => setDescription(e.currentTarget.value)}
          />
        </div>
        <div>
          <label class="label" for="edit-account">{t("tasks.account")}</label>
          <select
            id="edit-account"
            class="field"
            value={accountId()}
            onChange={(e) => setAccountId(e.currentTarget.value)}
          >
            <option value="">{t("tasks.noAccount")}</option>
            <For each={props.detail.accounts}>
              {(account) => <option value={account.id}>{account.name}</option>}
            </For>
          </select>
        </div>
        <Show when={error()}>
          {(message) => <p class="text-sm font-medium text-danger">{message()}</p>}
        </Show>
        <div class="flex gap-2 pt-1">
          <button type="button" class="btn-ghost flex-1" onClick={props.onClose}>
            {t("common.close")}
          </button>
          <button type="submit" class="btn-primary flex-1" disabled={busy()}>
            {busy() ? t("common.loading") : t("settings.save")}
          </button>
        </div>
      </form>
    </Sheet>
  );
}
