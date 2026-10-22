import { For, Show, createSignal, onMount } from "solid-js";
import { api, type Member, type TaskDetail as TaskDetailData } from "../../api";
import DurationField from "../../components/DurationField";
import TaskFormSheet from "../../components/TaskFormSheet";
import { dur, todayIso } from "../../format";
import { t } from "../../i18n";
import { closeTask, openTask, openTaskId } from "../../state";
import {
  deDate,
  effort,
  effortPercent,
  priorityClass,
  priorityLabel,
  statusClass,
  statusLabel,
} from "./util";

export default function TaskDetail() {
  const taskId = openTaskId;
  const [data, setData] = createSignal<TaskDetailData | null>(null);
  const [members, setMembers] = createSignal<Member[]>([]);
  const [error, setError] = createSignal<string | null>(null);
  const [showEdit, setShowEdit] = createSignal(false);

  const [comment, setComment] = createSignal("");
  const [subtaskTitle, setSubtaskTitle] = createSignal("");
  const [spentMinutes, setSpentMinutes] = createSignal<number | null>(60);
  const [spentOn, setSpentOn] = createSignal(todayIso());
  const [note, setNote] = createSignal("");
  const [busy, setBusy] = createSignal(false);

  async function load() {
    const id = taskId();
    if (!id) return;
    try {
      setData(await api.task(id));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  onMount(() => {
    void load();
    api.members().then(setMembers).catch(() => setMembers([]));
  });

  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    try {
      await action();
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function addComment(e: Event) {
    e.preventDefault();
    const id = taskId();
    if (!id || !comment().trim()) return;
    const body = comment();
    setComment("");
    await run(() => api.addComment(id, body));
  }

  async function addSubtask(e: Event) {
    e.preventDefault();
    const detail = data();
    if (!detail || !subtaskTitle().trim()) return;
    const title = subtaskTitle();
    setSubtaskTitle("");
    await run(() =>
      api.createTask({
        project_id: detail.task.project_id,
        parent_id: detail.task.id,
        title,
      }),
    );
  }

  async function addTime(e: Event) {
    e.preventDefault();
    const id = taskId();
    const value = spentMinutes();
    if (!id || !value || value <= 0) return;
    await run(() => api.addTaskTime(id, { minutes: value, note: note(), spent_on: spentOn() }));
    setNote("");
    setSpentMinutes(null);
  }

  async function removeTask() {
    const detail = data();
    if (!detail) return;
    if (!window.confirm(t("tasks.deleteTask"))) return;
    try {
      await api.deleteTask(detail.task.id);
      closeTask();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div class="space-y-4">
      <button
        type="button"
        class="text-xs font-semibold uppercase tracking-wide text-muted hover:text-foreground"
        onClick={closeTask}
      >
        ← {t("tasks.project")}
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
                  <div class="mb-1 flex flex-wrap items-center gap-1.5">
                    <span class={`rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase ${priorityClass(detail().task.priority)}`}>
                      {priorityLabel(detail().task.priority)}
                    </span>
                    <span class={`rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase ${statusClass(detail().task.status)}`}>
                      {statusLabel(detail().task.status)}
                    </span>
                    <span class="text-[10px] uppercase tracking-wide text-muted">
                      {detail().task.project_name}
                    </span>
                  </div>
                  <h1 class="font-display text-2xl font-bold uppercase leading-tight">
                    {detail().task.title}
                  </h1>
                  <Show when={detail().task.description}>
                    <p class="mt-2 whitespace-pre-wrap text-sm text-muted">
                      {detail().task.description}
                    </p>
                  </Show>
                </div>
                <div class="flex shrink-0 gap-1">
                  <button type="button" class="btn-ghost px-2 py-1 text-xs" onClick={() => setShowEdit(true)}>
                    {t("tasks.edit")}
                  </button>
                  <button
                    type="button"
                    class="rounded-lg border border-danger/40 px-2 py-1 text-xs font-semibold text-danger hover:bg-danger/10"
                    disabled={busy()}
                    onClick={() => void removeTask()}
                  >
                    {t("tasks.delete")}
                  </button>
                </div>
              </div>

              <div class="mt-3 grid grid-cols-2 gap-x-4 gap-y-1 text-xs text-muted sm:grid-cols-4">
                <span>
                  {t("tasks.assignee")}:{" "}
                  <b class="text-foreground">{detail().task.assignee_name ?? t("tasks.unassigned")}</b>
                </span>
                <span>
                  {t("tasks.start")}: <b class="text-foreground">{deDate(detail().task.start_date)}</b>
                </span>
                <span>
                  {t("tasks.due")}: <b class="text-foreground">{deDate(detail().task.due_date)}</b>
                </span>
                <span>
                  {t("tasks.estimate")}:{" "}
                  <b class="text-foreground">{effort(detail().task.estimate_minutes)}</b>
                </span>
              </div>
            </section>

            {/* effort */}
            <section class="card space-y-2 p-4">
              <div class="flex items-baseline justify-between">
                <h2 class="font-display text-lg font-bold uppercase">{t("tasks.actual")}</h2>
                <span class="text-sm">
                  <b>{effort(detail().task.spent_minutes)}</b>
                  <Show when={detail().task.estimate_minutes}>
                    <span class="text-muted"> / {effort(detail().task.estimate_minutes)}</span>
                  </Show>
                </span>
              </div>
              <Show when={detail().task.estimate_minutes}>
                <div class="h-2 rounded-full bg-soft">
                  <div
                    class={`h-2 rounded-full ${
                      detail().task.spent_minutes > (detail().task.estimate_minutes ?? 0)
                        ? "bg-danger"
                        : "bg-primary"
                    }`}
                    style={{
                      width: `${effortPercent(
                        detail().task.spent_minutes,
                        detail().task.estimate_minutes ?? 0,
                      )}%`,
                    }}
                  />
                </div>
              </Show>

              <form onSubmit={addTime} class="space-y-2 pt-1">
                <DurationField value={spentMinutes()} onChange={setSpentMinutes} />
                <div class="flex flex-wrap items-end gap-2">
                  <div class="w-40">
                    <label class="label" for="time-date">{t("tasks.date")}</label>
                    <input
                      id="time-date"
                      type="date"
                      class="field"
                      value={spentOn()}
                      onInput={(e) => setSpentOn(e.currentTarget.value)}
                    />
                  </div>
                  <div class="min-w-40 flex-1">
                    <label class="label" for="time-note">{t("tasks.note")}</label>
                    <input
                      id="time-note"
                      class="field"
                      value={note()}
                      onInput={(e) => setNote(e.currentTarget.value)}
                    />
                  </div>
                  <button
                    type="submit"
                    class="btn-primary h-11"
                    disabled={busy() || !spentMinutes()}
                  >
                    + {t("tasks.addTime")}
                  </button>
                </div>
              </form>

              <div class="divide-y divide-line">
                <For each={detail().time_entries}>
                  {(entry) => (
                    <div class="flex items-center justify-between gap-2 py-1.5 text-sm">
                      <div class="min-w-0">
                        <span class="font-semibold tabular-nums">{dur(entry.minutes * 60_000)}</span>
                        <span class="ml-2 text-xs text-muted">{deDate(entry.spent_on)} · {entry.user_name}</span>
                        <Show when={entry.note}>
                          <div class="truncate text-xs text-muted">{entry.note}</div>
                        </Show>
                      </div>
                      <button
                        type="button"
                        class="rounded px-2 text-xs text-muted hover:text-danger"
                        onClick={() => void run(() => api.deleteTaskTime(detail().task.id, entry.id))}
                      >
                        ✕
                      </button>
                    </div>
                  )}
                </For>
                <Show when={detail().time_entries.length === 0}>
                  <p class="py-1 text-xs text-muted">{t("common.noData")}</p>
                </Show>
              </div>
            </section>

            {/* subtasks */}
            <section class="card space-y-2 p-4">
              <h2 class="font-display text-lg font-bold uppercase">{t("tasks.subtasks")}</h2>
              <div class="divide-y divide-line">
                <For each={detail().subtasks}>
                  {(subtask) => (
                    <button
                      type="button"
                      class="flex w-full items-center justify-between gap-2 py-1.5 text-left text-sm"
                      onClick={() => openTask(subtask.id)}
                    >
                      <span class={subtask.status === "done" ? "text-muted line-through" : ""}>
                        {subtask.title}
                      </span>
                      <span class="text-xs text-muted">
                        {statusLabel(subtask.status)}
                        <Show when={subtask.estimate_minutes}>
                          {" · "}
                          {effort(subtask.estimate_minutes)}
                        </Show>
                      </span>
                    </button>
                  )}
                </For>
              </div>
              <form onSubmit={addSubtask} class="flex gap-2">
                <input
                  class="field flex-1"
                  placeholder={t("tasks.addSubtask")}
                  value={subtaskTitle()}
                  onInput={(e) => setSubtaskTitle(e.currentTarget.value)}
                />
                <button type="submit" class="btn-ghost" disabled={busy()}>
                  {t("common.add")}
                </button>
              </form>
            </section>

            {/* comments */}
            <section class="card space-y-2 p-4">
              <h2 class="font-display text-lg font-bold uppercase">{t("tasks.comments")}</h2>
              <div class="space-y-2">
                <For each={detail().comments}>
                  {(item) => (
                    <div class="rounded-lg bg-soft/60 px-3 py-2">
                      <div class="flex items-baseline justify-between gap-2">
                        <span class="text-xs font-semibold">{item.author_name}</span>
                        <div class="flex items-center gap-2">
                          <span class="text-[10px] text-muted">
                            {new Date(item.created_at).toLocaleString("de-DE", {
                              dateStyle: "short",
                              timeStyle: "short",
                            })}
                          </span>
                          <button
                            type="button"
                            class="text-xs text-muted hover:text-danger"
                            onClick={() => void run(() => api.deleteComment(detail().task.id, item.id))}
                          >
                            ✕
                          </button>
                        </div>
                      </div>
                      <p class="mt-1 whitespace-pre-wrap text-sm">{item.body}</p>
                    </div>
                  )}
                </For>
                <Show when={detail().comments.length === 0}>
                  <p class="text-xs text-muted">{t("tasks.noComments")}</p>
                </Show>
              </div>
              <form onSubmit={addComment} class="flex items-end gap-2">
                <textarea
                  rows={2}
                  class="field flex-1 resize-none"
                  placeholder={t("tasks.addComment")}
                  value={comment()}
                  onInput={(e) => setComment(e.currentTarget.value)}
                />
                <button type="submit" class="btn-primary h-11" disabled={busy()}>
                  {t("common.add")}
                </button>
              </form>
            </section>

            <TaskFormSheet
              open={showEdit()}
              projectId={detail().task.project_id}
              task={detail().task}
              members={members()}
              onClose={() => setShowEdit(false)}
              onSaved={() => void load()}
            />
          </>
        )}
      </Show>
    </div>
  );
}
