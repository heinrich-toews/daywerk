import type { TaskPriority, TaskStatus } from "../../api";
import { dur } from "../../format";
import { t } from "../../i18n";

export function priorityLabel(priority: TaskPriority): string {
  return t(`priority.${priority}` as "priority.low");
}

export function priorityClass(priority: TaskPriority): string {
  switch (priority) {
    case "urgent":
      return "bg-danger/15 text-danger";
    case "high":
      return "bg-amber-500/15 text-amber-700";
    case "low":
      return "bg-line text-muted";
    default:
      return "bg-soft text-primary";
  }
}

export function statusLabel(status: TaskStatus): string {
  return t(`status.${status}` as "status.open");
}

export function statusClass(status: TaskStatus): string {
  switch (status) {
    case "done":
      return "bg-ok/10 text-ok";
    case "in_progress":
      return "bg-primary/10 text-primary";
    case "cancelled":
      return "bg-line text-muted line-through";
    default:
      return "bg-soft text-muted";
  }
}

/** Minutes as `H:MM`. */
export function effort(minutes: number | null | undefined): string {
  return dur((minutes ?? 0) * 60_000);
}

/** Milliseconds from the punch clock as `H:MM`. */
export function booked(ms: number | null | undefined): string {
  return dur(ms ?? 0);
}

export const PRIORITIES: TaskPriority[] = ["low", "normal", "high", "urgent"];
export const STATUSES: TaskStatus[] = ["open", "in_progress", "done", "cancelled"];

/** `2026-10-21` → `21.10.2026` */
export function deDate(iso: string | null | undefined): string {
  if (!iso) return "–";
  const parts = iso.split("-");
  return parts.length === 3 ? `${parts[2]}.${parts[1]}.${parts[0]}` : iso;
}

/** Estimate vs effort in percent, clamped to 0..100 for a progress bar. */
export function effortPercent(spent: number, estimate: number): number {
  if (estimate <= 0) return spent > 0 ? 100 : 0;
  return Math.min(100, Math.round((spent / estimate) * 100));
}

/** Add a chip delta to a duration, clamped to the maximum (default 24 h). */
export function addDuration(
  current: number | null,
  delta: number,
  maxMinutes = 24 * 60,
): number {
  return Math.min(maxMinutes, (current ?? 0) + delta);
}

/** Hours and minutes from the two input fields, clamped to a valid duration.
 * Extra minutes roll over (`1 h 75 min` → `2:15`), which is what users expect
 * when they type instead of using the chips. */
export function toMinutes(hours: number, minutes: number, maxMinutes = 24 * 60): number {
  if (!Number.isFinite(hours) || !Number.isFinite(minutes)) return 0;
  const h = Math.max(0, Math.floor(hours));
  const m = Math.max(0, Math.floor(minutes));
  return Math.min(maxMinutes, h * 60 + m);
}
