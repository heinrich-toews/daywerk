import { For, Show, createEffect, createSignal } from "solid-js";
import { api, type DayView as DayViewData, type Segment } from "../api";
import NoteSheet from "../components/NoteSheet";
import Sheet from "../components/Sheet";
import { TrashIcon } from "../components/Icons";
import {
  balanceClass,
  dur,
  longDate,
  monthStartIso,
  timeOfDay,
  todayIso,
} from "../format";
import { locale, t } from "../i18n";
import { me } from "../state";

type Editing = { id: string; note: string; material: string };

export default function History() {
  const [from, setFrom] = createSignal(monthStartIso());
  const [to, setTo] = createSignal(todayIso());
  const [items, setItems] = createSignal<DayViewData[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [editing, setEditing] = createSignal<Editing | null>(null);
  const [deleting, setDeleting] = createSignal<{ segment: Segment; date: string } | null>(null);
  const [deleteBusy, setDeleteBusy] = createSignal(false);

  async function load(f: string, until: string) {
    setLoading(true);
    try {
      setItems(await api.days(f, until));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  createEffect(() => {
    const f = from();
    const until = to();
    void load(f, until);
  });

  const accountName = (id: string) => me()?.accounts.find((a) => a.id === id)?.name ?? "—";
  const targetOf = (entry: DayViewData) => entry.target_minutes * 60_000;

  const canEdit = (segment: Segment, date: string) => {
    const user = me()?.user;
    if (!user) return false;
    if (user.role === "admin" || user.role === "office") return true;
    return segment.user_id === user.id && date === todayIso();
  };

  /** Admins and office may remove any booking; employees only their own. */
  const canDelete = (segment: Segment) => {
    const user = me()?.user;
    if (!user) return false;
    if (user.role === "admin" || user.role === "office") return true;
    return segment.user_id === user.id;
  };

  async function confirmDelete() {
    const target = deleting();
    if (!target) return;
    setDeleteBusy(true);
    setError(null);
    try {
      await api.deleteSegment(target.segment.id);
      setDeleting(null);
      await load(from(), to());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setDeleteBusy(false);
    }
  }

  async function saveNote(note: string, material: string) {
    const current = editing();
    if (!current) return;
    setEditing(null);
    try {
      await api.updateSegmentNote(current.id, note, material);
      await load(from(), to());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

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
          fallback={<p class="card p-6 text-center text-sm text-muted">{t("history.empty")}</p>}
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
                          entry.totals.work_ms -
                            (entry.segments.length > 0 ? targetOf(entry) : 0),
                        )}`}
                      >
                        {entry.segments.length > 0
                          ? dur(entry.totals.work_ms - targetOf(entry))
                          : t("common.none")}
                      </span>
                      <Show when={entry.day.status === "closed"}>
                        <span class="text-[10px] uppercase text-muted">{t("punch.closed")}</span>
                      </Show>
                    </div>
                  </header>

                  <div class="mt-2 space-y-1.5">
                    <For each={entry.segments}>
                      {(segment) => (
                        <div class="rounded-md bg-soft/60 px-2 py-1.5 text-sm">
                          <div class="flex items-baseline justify-between gap-2">
                            <span class="min-w-0 truncate font-medium">
                              {accountName(segment.account_id)}
                            </span>
                            <span class="flex shrink-0 items-center gap-2 tabular-nums text-muted">
                              {timeOfDay(segment.started_at, locale())}–
                              {segment.ended_at ? timeOfDay(segment.ended_at, locale()) : "…"}
                              <span class="font-semibold text-foreground">
                                {dur(
                                  (segment.ended_at ?? segment.started_at) - segment.started_at,
                                )}
                              </span>
                              <Show when={canEdit(segment, entry.day.date)}>
                                <button
                                  type="button"
                                  class="rounded px-1 text-muted hover:bg-card hover:text-primary"
                                  title={t("note.edit")}
                                  onClick={() =>
                                    setEditing({
                                      id: segment.id,
                                      note: segment.note,
                                      material: segment.material,
                                    })
                                  }
                                >
                                  ✎
                                </button>
                              </Show>
                              <Show when={canDelete(segment)}>
                                <button
                                  type="button"
                                  class="rounded px-1 text-muted hover:bg-card hover:text-danger"
                                  title={t("history.deleteBooking")}
                                  onClick={() =>
                                    setDeleting({ segment, date: entry.day.date })
                                  }
                                >
                                  <TrashIcon class="h-4 w-4" />
                                </button>
                              </Show>
                            </span>
                          </div>
                          <Show when={segment.note}>
                            <p class="mt-1 text-xs text-muted">
                              <span class="font-semibold uppercase">{t("note.note")}:</span>{" "}
                              {segment.note}
                            </p>
                          </Show>
                          <Show when={segment.material}>
                            <p class="text-xs text-muted">
                              <span class="font-semibold uppercase">{t("note.material")}:</span>{" "}
                              {segment.material}
                            </p>
                          </Show>
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

      <Show when={editing()} keyed>
        {(current) => (
          <NoteSheet
            open={true}
            title={t("note.edit")}
            initialNote={current.note}
            initialMaterial={current.material}
            onClose={() => setEditing(null)}
            onSave={saveNote}
          />
        )}
      </Show>

      <Sheet
        open={deleting() !== null}
        title={t("history.deleteBooking")}
        onClose={() => {
          if (!deleteBusy()) setDeleting(null);
        }}
      >
        <Show when={deleting()}>
          {(target) => (
            <div class="space-y-4">
              <p class="text-sm">
                <b>{accountName(target().segment.account_id)}</b> ·{" "}
                {longDate(target().date, locale())} ·{" "}
                {timeOfDay(target().segment.started_at, locale())}–
                {target().segment.ended_at
                  ? timeOfDay(target().segment.ended_at!, locale())
                  : "…"}{" "}
                (
                {dur(
                  (target().segment.ended_at ?? target().segment.started_at) -
                    target().segment.started_at,
                )}
                )
              </p>
              <p class="text-sm font-semibold text-danger">
                {t("history.deleteWarning")}
              </p>
              <div class="flex gap-2">
                <button
                  type="button"
                  class="btn-ghost flex-1"
                  disabled={deleteBusy()}
                  onClick={() => setDeleting(null)}
                >
                  {t("common.cancel")}
                </button>
                <button
                  type="button"
                  class="inline-flex flex-1 items-center justify-center gap-2 rounded-lg bg-danger px-4 py-2.5 font-semibold text-white transition-colors hover:opacity-90 disabled:opacity-50"
                  disabled={deleteBusy()}
                  onClick={() => void confirmDelete()}
                >
                  {deleteBusy() ? t("common.loading") : t("admin.delete")}
                </button>
              </div>
            </div>
          )}
        </Show>
      </Sheet>
    </div>
  );
}
