import { Show } from "solid-js";
import type { TaskRow } from "../api";
import { t } from "../i18n";
import { deDate, effort, priorityClass, priorityLabel, statusClass, statusLabel } from "../views/tasks/util";

/** One task in a list. Subtasks are summarised, not expanded. */
export default function TaskRowItem(props: {
  row: TaskRow;
  showProject?: boolean;
  onOpen: () => void;
}) {
  const overdue = () =>
    !!props.row.due_date &&
    props.row.status !== "done" &&
    props.row.status !== "cancelled" &&
    props.row.due_date < new Date().toISOString().slice(0, 10);

  return (
    <button
      type="button"
      onClick={props.onOpen}
      class="flex w-full flex-col gap-1 rounded-lg border border-line bg-card px-3 py-2 text-left transition-colors hover:border-primary/40"
    >
      <div class="flex items-start justify-between gap-2">
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-1.5">
            <span class={`rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase ${priorityClass(props.row.priority)}`}>
              {priorityLabel(props.row.priority)}
            </span>
            <Show when={props.row.status !== "open"}>
              <span class={`rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase ${statusClass(props.row.status)}`}>
                {statusLabel(props.row.status)}
              </span>
            </Show>
            <Show when={props.showProject}>
              <span class="text-[10px] uppercase tracking-wide text-muted">
                {props.row.project_name}
              </span>
            </Show>
          </div>
          <div class="mt-0.5 truncate text-sm font-semibold">{props.row.title}</div>
        </div>
        <div class="shrink-0 text-right text-xs text-muted">
          <div class={overdue() ? "font-semibold text-danger" : ""}>
            {props.row.due_date ? deDate(props.row.due_date) : "–"}
          </div>
          <Show when={props.row.assignee_name}>
            <div class="truncate">{props.row.assignee_name}</div>
          </Show>
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-3 text-[11px] text-muted">
        <span>
          {t("tasks.spent")}: <b class="text-foreground">{effort(props.row.spent_minutes)}</b>
          <Show when={props.row.estimate_minutes}>
            {" / "}
            {effort(props.row.estimate_minutes)}
          </Show>
        </span>
        <Show when={props.row.subtask_count > 0}>
          <span>
            {props.row.done_subtasks}/{props.row.subtask_count} {t("tasks.subtasks")}
          </span>
        </Show>
        <Show when={props.row.comment_count > 0}>
          <span>💬 {props.row.comment_count}</span>
        </Show>
        <Show when={overdue()}>
          <span class="font-semibold text-danger">{t("tasks.overdue")}</span>
        </Show>
      </div>
    </button>
  );
}
