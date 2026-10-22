import { For, Show, createEffect, createSignal } from "solid-js";
import { dur } from "../format";
import { t } from "../i18n";
import { addDuration, toMinutes } from "../views/tasks/util";

/**
 * Duration input for durations (`H:MM`), with additive quarter-hour chips.
 *
 * Values are minutes — the unit the API stores. The chips *add* to the current
 * value, so 2:45 is `+1 h`, `+1 h`, `+45`.
 */
export default function DurationField(props: {
  value: number | null;
  onChange: (minutes: number | null) => void;
  /** Estimates may stay empty ("no estimate"); effort may not. */
  allowEmpty?: boolean;
  maxMinutes?: number;
}) {
  const max = () => props.maxMinutes ?? 24 * 60;

  const [hours, setHours] = createSignal("");
  const [minutes, setMinutes] = createSignal("");
  // The last value this component pushed out, so the sync effect does not
  // overwrite fields the user is currently editing.
  let lastEmitted: number | null | undefined;

  createEffect(() => {
    const value = props.value ?? null;
    if (value === lastEmitted) return;
    setHours(value == null ? "" : String(Math.floor(value / 60)));
    setMinutes(value == null ? "" : String(value % 60));
    lastEmitted = value;
  });

  function emit(nextHours: string, nextMinutes: string) {
    const h = nextHours.trim() === "" ? 0 : Number(nextHours);
    const m = nextMinutes.trim() === "" ? 0 : Number(nextMinutes);
    if (!Number.isFinite(h) || !Number.isFinite(m)) return;

    const total = toMinutes(h, m, max());
    const next = total === 0 && props.allowEmpty ? null : total;
    lastEmitted = next;
    props.onChange(next);
    // reflect normalisation (rollover, cap) in the two fields
    setHours(String(Math.floor(total / 60)));
    setMinutes(String(total % 60));
  }

  function add(delta: number) {
    const next = addDuration(props.value, delta, max());
    lastEmitted = next;
    props.onChange(next);
    setHours(String(Math.floor(next / 60)));
    setMinutes(String(next % 60));
  }

  function reset() {
    const next = props.allowEmpty ? null : 0;
    lastEmitted = next;
    props.onChange(next);
    setHours("");
    setMinutes("");
  }

  const chips: { label: string; delta: number }[] = [
    { label: "+15", delta: 15 },
    { label: "+30", delta: 30 },
    { label: "+45", delta: 45 },
    { label: t("tasks.oneHour"), delta: 60 },
  ];

  return (
    <div class="space-y-2">
      <div class="grid grid-cols-4 gap-1.5">
        <For each={chips}>
          {(chip) => (
            <button
              type="button"
              onClick={() => add(chip.delta)}
              class="h-11 rounded-lg border border-line bg-card text-sm font-semibold text-primary transition-colors hover:border-primary/40 hover:bg-soft active:scale-[0.98]"
            >
              {chip.label}
            </button>
          )}
        </For>
      </div>

      <div class="flex items-end gap-2">
        <div class="w-24">
          <label class="label" for="duration-hours">
            {t("tasks.hours")}
          </label>
          <input
            id="duration-hours"
            type="number"
            inputmode="numeric"
            min="0"
            max="24"
            step="1"
            class="field tabular-nums"
            placeholder="0"
            value={hours()}
            onInput={(e) => {
              setHours(e.currentTarget.value);
              emit(e.currentTarget.value, minutes());
            }}
          />
        </div>
        <div class="w-24">
          <label class="label" for="duration-minutes">
            {t("tasks.minutes")}
          </label>
          <input
            id="duration-minutes"
            type="number"
            inputmode="numeric"
            min="0"
            max="59"
            step="1"
            class="field tabular-nums"
            placeholder="0"
            value={minutes()}
            onInput={(e) => {
              setMinutes(e.currentTarget.value);
              emit(hours(), e.currentTarget.value);
            }}
          />
        </div>
        <div class="flex-1 pb-2 text-right">
          <span class="font-display text-lg font-bold tabular-nums">
            {dur((props.value ?? 0) * 60_000)}
          </span>
          <Show when={props.value != null || props.allowEmpty}>
            <button
              type="button"
              onClick={reset}
              title={t("tasks.reset")}
              class="ml-2 rounded px-1 text-muted hover:text-danger"
            >
              ↺
            </button>
          </Show>
        </div>
      </div>
    </div>
  );
}
