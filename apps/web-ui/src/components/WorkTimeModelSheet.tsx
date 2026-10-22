import { For, Show, createSignal, onMount } from "solid-js";
import { api, type User, type WorkTimeModel } from "../api";
import { dur, todayIso } from "../format";
import { t } from "../i18n";
import Sheet from "./Sheet";

/** Contract history of one employee, with a form to start a new period. */
export default function WorkTimeModelSheet(props: { user: User; onClose: () => void }) {
  const [models, setModels] = createSignal<WorkTimeModel[]>([]);
  const [validFrom, setValidFrom] = createSignal(todayIso());
  const [hours, setHours] = createSignal("40");
  const [days, setDays] = createSignal("5");
  const [note, setNote] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function load() {
    try {
      setModels(await api.workTimeModels(props.user.id));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  onMount(() => {
    void load();
  });

  const parsed = () => {
    const h = Number(hours().replace(",", "."));
    const d = Number(days());
    return { h, d };
  };

  const dailyMinutes = () => {
    const { h, d } = parsed();
    if (!h || !d) return 0;
    return Math.round((h * 60) / d);
  };

  async function add(e: Event) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const { h, d } = parsed();
      await api.createWorkTimeModel(props.user.id, {
        valid_from: validFrom(),
        weekly_hours: h,
        workdays_per_week: d,
        note: note(),
      });
      setNote("");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  const lastId = () => models().at(-1)?.id;

  return (
    <Sheet open={true} title={`${t("model.title")} · ${props.user.name}`} onClose={props.onClose}>
      <div class="space-y-3">
        <div class="divide-y divide-line">
          <For each={models()}>
            {(model) => (
              <div class="flex flex-wrap items-baseline justify-between gap-2 py-2 text-sm">
                <div class="min-w-0">
                  <span class="font-semibold tabular-nums">{model.valid_from}</span>
                  <Show when={model.id === lastId()}>
                    <span class="ml-2 rounded bg-soft px-1.5 py-0.5 text-[10px] font-semibold uppercase text-primary">
                      {t("model.current")}
                    </span>
                  </Show>
                  <Show when={model.note}>
                    <div class="text-xs text-muted">{model.note}</div>
                  </Show>
                </div>
                <div class="text-right">
                  <div class="font-semibold tabular-nums">
                    {model.weekly_hours} h · {model.workdays_per_week} {t("model.daysShort")}
                  </div>
                  <div class="text-xs text-muted">
                    {dur(Math.round((model.weekly_hours * 60) / model.workdays_per_week) * 60_000)}
                    {" / "}
                    {t("model.daysShort")}
                  </div>
                </div>
              </div>
            )}
          </For>
          <Show when={models().length === 0}>
            <p class="py-2 text-sm text-muted">{t("model.none")}</p>
          </Show>
        </div>

        <form onSubmit={add} class="space-y-2 border-t border-line pt-3">
          <h3 class="text-xs font-semibold uppercase tracking-wide text-muted">
            {t("model.add")}
          </h3>
          <div class="grid grid-cols-2 gap-2">
            <div>
              <label class="label" for="model-from">
                {t("model.from")}
              </label>
              <input
                id="model-from"
                type="date"
                class="field"
                required
                value={validFrom()}
                onInput={(e) => setValidFrom(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="label" for="model-note">
                {t("model.note")}
              </label>
              <input
                id="model-note"
                class="field"
                value={note()}
                onInput={(e) => setNote(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="label" for="model-hours">
                {t("model.weeklyHours")}
              </label>
              <input
                id="model-hours"
                type="number"
                min="0.25"
                max="80"
                step="0.25"
                class="field"
                required
                value={hours()}
                onInput={(e) => setHours(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="label" for="model-days">
                {t("model.workdays")}
              </label>
              <input
                id="model-days"
                type="number"
                min="1"
                max="7"
                step="1"
                class="field"
                required
                value={days()}
                onInput={(e) => setDays(e.currentTarget.value)}
              />
            </div>
          </div>

          <p class="text-sm text-muted">
            {t("model.daily")}:{" "}
            <b class="text-foreground tabular-nums">{dur(dailyMinutes() * 60_000)}</b>
          </p>
          <p class="text-[11px] text-muted">{t("model.retro")}</p>

          <Show when={error()}>
            {(message) => <p class="text-sm font-medium text-danger">{message()}</p>}
          </Show>

          <div class="flex gap-2">
            <button type="button" class="btn-ghost flex-1" onClick={props.onClose}>
              {t("common.close")}
            </button>
            <button type="submit" class="btn-primary flex-1" disabled={busy()}>
              {busy() ? t("common.loading") : t("common.add")}
            </button>
          </div>
        </form>
      </div>
    </Sheet>
  );
}
