import { For, Show, createSignal } from "solid-js";
import { api, type Member, type Task, type TaskPriority, type TaskStatus } from "../api";
import DurationField from "./DurationField";
import { t } from "../i18n";
import { PRIORITIES, STATUSES, priorityLabel, statusLabel } from "../views/tasks/util";
import Sheet from "./Sheet";

/** Create or edit a task. Mount it keyed so the fields reset per task. */
export default function TaskFormSheet(props: {
  open: boolean;
  projectId: string;
  task?: Task | null;
  members: Member[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const editing = () => !!props.task;
  const [title, setTitle] = createSignal(props.task?.title ?? "");
  const [description, setDescription] = createSignal(props.task?.description ?? "");
  const [assignee, setAssignee] = createSignal(props.task?.assignee_id ?? "");
  const [start, setStart] = createSignal(props.task?.start_date ?? "");
  const [due, setDue] = createSignal(props.task?.due_date ?? "");
  const [priority, setPriority] = createSignal<TaskPriority>(props.task?.priority ?? "normal");
  const [status, setStatus] = createSignal<TaskStatus>(props.task?.status ?? "open");
  const [estimateMinutes, setEstimateMinutes] = createSignal<number | null>(
    props.task?.estimate_minutes ?? null,
  );
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function save(e: Event) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const body = {
        title: title().trim(),
        description: description().trim(),
        assignee_id: assignee() || null,
        start_date: start() || null,
        due_date: due() || null,
        priority: priority(),
        estimate_minutes: estimateMinutes(),
      };
      if (props.task) {
        await api.updateTask(props.task.id, { ...body, status: status() });
      } else {
        await api.createTask({ ...body, project_id: props.projectId });
      }
      props.onSaved();
      props.onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Sheet
      open={props.open}
      title={editing() ? t("tasks.edit") : t("tasks.newTask")}
      onClose={props.onClose}
    >
      <form onSubmit={save} class="space-y-2">
        <div>
          <label class="label" for="task-title">{t("nav.tasks")}</label>
          <input
            id="task-title"
            class="field"
            required
            value={title()}
            onInput={(e) => setTitle(e.currentTarget.value)}
          />
        </div>
        <div>
          <label class="label" for="task-desc">{t("tasks.description")}</label>
          <textarea
            id="task-desc"
            rows={2}
            class="field resize-none"
            value={description()}
            onInput={(e) => setDescription(e.currentTarget.value)}
          />
        </div>
        <div class="grid grid-cols-2 gap-2">
          <div>
            <label class="label" for="task-assignee">{t("tasks.assignee")}</label>
            <select
              id="task-assignee"
              class="field"
              value={assignee()}
              onChange={(e) => setAssignee(e.currentTarget.value)}
            >
              <option value="">{t("tasks.unassigned")}</option>
              <For each={props.members}>{(member) => <option value={member.id}>{member.name}</option>}</For>
            </select>
          </div>
          <div>
            <label class="label" for="task-priority">{t("tasks.priority")}</label>
            <select
              id="task-priority"
              class="field"
              value={priority()}
              onChange={(e) => setPriority(e.currentTarget.value as TaskPriority)}
            >
              <For each={PRIORITIES}>
                {(value) => <option value={value}>{priorityLabel(value)}</option>}
              </For>
            </select>
          </div>
          <div>
            <label class="label" for="task-start">{t("tasks.start")}</label>
            <input
              id="task-start"
              type="date"
              class="field"
              value={start()}
              onInput={(e) => setStart(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="label" for="task-due">{t("tasks.due")}</label>
            <input
              id="task-due"
              type="date"
              class="field"
              value={due()}
              onInput={(e) => setDue(e.currentTarget.value)}
            />
          </div>
          <Show when={editing()}>
            <div>
              <label class="label" for="task-status">{t("tasks.status")}</label>
              <select
                id="task-status"
                class="field"
                value={status()}
                onChange={(e) => setStatus(e.currentTarget.value as TaskStatus)}
              >
                <For each={STATUSES}>{(value) => <option value={value}>{statusLabel(value)}</option>}</For>
              </select>
            </div>
          </Show>
        </div>

        <div>
          <span class="label">{t("tasks.estimate")}</span>
          <DurationField
            value={estimateMinutes()}
            onChange={setEstimateMinutes}
            allowEmpty={true}
          />
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
