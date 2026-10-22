import { For, Show, createSignal, onCleanup, onMount, type JSX } from "solid-js";
import { api, ApiError } from "../api";
import { clockOffset, elapsedSince, serverTime } from "../clock";
import { Pause, Play, Stop } from "../components/Icons";
import NoteSheet from "../components/NoteSheet";
import Sheet from "../components/Sheet";
import { balanceClass, dur, durSeconds, longDate, timeOfDay } from "../format";
import { locale, t } from "../i18n";
import { punch, refresh, reloadToday, today, todayReceivedAt } from "../state";

function PunchButton(props: {
  label: string;
  icon: JSX.Element;
  variant: "work" | "break" | "stop";
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  const activeClass = () => {
    if (props.variant === "break") return "border-amber-400 bg-amber-400 text-white";
    if (props.variant === "stop") return "border-danger bg-danger text-white";
    return "border-primary bg-primary text-primary-foreground shadow-lg shadow-primary/25";
  };

  const idleClass = () => {
    if (props.variant === "stop") return "border-danger/30 bg-card text-danger";
    if (props.variant === "break") return "border-amber-400/30 bg-card text-amber-600";
    return "border-primary/20 bg-soft text-primary";
  };

  return (
    <button
      type="button"
      disabled={props.disabled}
      onClick={props.onClick}
      class={`flex h-24 flex-col items-center justify-center gap-1 rounded-xl border-2 font-display text-sm font-bold uppercase tracking-wide transition-all ${
        props.active ? activeClass() : idleClass()
      } ${props.disabled ? "opacity-35 saturate-0" : "active:scale-[0.98]"}`}
    >
      {props.icon}
      <span class="max-w-full truncate px-1">{props.label}</span>
    </button>
  );
}

type Pending = {
  action: "start" | "pause" | "stop";
  accountId?: string;
  accountName: string;
  from: number;
  to: number;
};

export default function Today() {
  const [tick, setTick] = createSignal(Date.now());
  const [error, setError] = createSignal<string | null>(null);
  const [pending, setPending] = createSignal<Pending | null>(null);
  const [showAdd, setShowAdd] = createSignal(false);
  const [newName, setNewName] = createSignal("");
  const [addBusy, setAddBusy] = createSignal(false);
  const [addError, setAddError] = createSignal<string | null>(null);

  onMount(() => {
    const id = window.setInterval(() => setTick(Date.now()), 1000);

    // Coming back to the tab (or to this view after a while) should show the
    // authoritative state, but never at the cost of a punch that is in flight.
    const onVisible = () => {
      if (document.visibilityState !== "visible") return;
      setTick(Date.now());
      void reloadToday().catch(() => undefined);
    };
    document.addEventListener("visibilitychange", onVisible);

    if (Date.now() - todayReceivedAt() > 60_000) {
      void reloadToday().catch(() => undefined);
    }

    onCleanup(() => {
      window.clearInterval(id);
      document.removeEventListener("visibilitychange", onVisible);
    });
  });

  /**
   * Clock offset between server and device, fixed at the moment the payload
   * arrived. Deriving it from `Date.now()` inside render would make the offset
   * drift on every remount — the running timer would jump back to zero.
   */
  const offset = () => {
    const data = today();
    const received = todayReceivedAt();
    if (!data || !received) return 0;
    return clockOffset(data.now, received);
  };

  const serverNow = () => serverTime(tick(), offset());
  const accounts = () => today()?.accounts ?? [];
  const primary = () =>
    accounts().find((a) => a.is_primary) ?? accounts().find((a) => a.kind === "work");
  const others = () =>
    accounts().filter((a) => a.kind === "work" && a.id !== primary()?.id);
  const active = () => today()?.active ?? null;
  const isActive = (id?: string) => !!id && active()?.account_id === id;
  const onBreak = () => active()?.kind === "break";
  const dayOpen = () => today()?.day?.day.status === "open";
  const canPause = () => !!active() && !onBreak();
  const canStop = () => dayOpen();

  const elapsed = () => {
    const a = active();
    if (!a) return 0;
    return elapsedSince(a.started_at, tick(), offset());
  };

  const liveTotals = () => {
    const data = today();
    if (!data?.day) return { work: 0, break: 0, span: 0 };
    let work = 0;
    let brk = 0;
    for (const segment of data.day.segments) {
      const end = segment.ended_at ?? serverNow();
      const ms = Math.max(0, end - segment.started_at);
      const account = data.accounts.find((a) => a.id === segment.account_id);
      if (account?.kind === "break") brk += ms;
      else work += ms;
    }
    return { work, break: brk, span: work + brk };
  };

  async function go(
    action: "start" | "pause" | "stop",
    accountId?: string,
    note?: string,
    material?: string,
  ) {
    setError(null);
    try {
      await punch(action, accountId, note, material);
    } catch (e) {
      setError(`${t("common.error")}: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  /**
   * Every transition that closes a *work* segment asks for a note first.
   * Closing a break (resuming or stopping while paused) needs no note.
   */
  function request(action: "start" | "pause" | "stop", accountId?: string) {
    const a = active();
    if (a && a.kind === "work") {
      setPending({
        action,
        accountId,
        accountName: a.account_name,
        from: a.started_at,
        to: serverNow(),
      });
      return;
    }
    void go(action, accountId);
  }

  async function createAccount(e: Event) {
    e.preventDefault();
    const name = newName().trim();
    if (!name) return;
    setAddBusy(true);
    setAddError(null);
    try {
      await api.createAccount({ name, kind: "work" });
      setNewName("");
      setShowAdd(false);
      await refresh();
    } catch (err) {
      setAddError(
        err instanceof ApiError ? err.message : err instanceof Error ? err.message : String(err),
      );
    } finally {
      setAddBusy(false);
    }
  }

  return (
    <div class="space-y-4">
      <section class="flex items-start justify-between gap-3">
        <div>
          <div class="text-[11px] font-semibold uppercase tracking-wide text-muted">
            {today() ? longDate(today()!.date, locale()) : ""}
          </div>
          <div class="font-display text-5xl font-bold leading-none tabular-nums">
            {timeOfDay(serverNow(), locale())}
          </div>
        </div>
        <div class="shrink-0 text-right">
          <div class="text-[10px] font-semibold uppercase tracking-wide text-muted">
            {t("balance.month")}
          </div>
          <div
            class={`font-display text-2xl font-bold leading-tight tabular-nums ${balanceClass(
              today()?.balance_month.balance_ms ?? 0,
            )}`}
          >
            {dur(today()?.balance_month.balance_ms ?? 0)}
          </div>
          <div class="text-[10px] text-muted">
            {t("balance.total")} {dur(today()?.balance_total.balance_ms ?? 0)}
          </div>
        </div>
      </section>

      <Show when={error()}>
        {(message) => (
          <p class="rounded-lg bg-danger/10 px-3 py-2 text-sm font-medium text-danger">
            {message()}
          </p>
        )}
      </Show>

      <section class="card px-4 py-3 text-center">
        <Show
          when={active()}
          fallback={
            <div class="py-3">
              <div class="font-display text-2xl font-bold uppercase tracking-wide text-muted">
                {today()?.day?.day.status === "closed" ? t("punch.closed") : t("punch.idle")}
              </div>
              <div class="mt-1 text-xs text-muted">{t("punch.hint")}</div>
            </div>
          }
        >
          {(a) => (
            <div class="py-1">
              <div class="text-[11px] font-semibold uppercase tracking-wide text-muted">
                {t("punch.activeSince", { time: timeOfDay(a().started_at, locale()) })}
              </div>
              <div class="font-display text-5xl font-bold leading-tight tabular-nums text-primary">
                {durSeconds(elapsed())}
              </div>
              <div class="text-sm font-semibold uppercase tracking-wide text-muted">
                {a().account_name}
              </div>
            </div>
          )}
        </Show>
      </section>

      <Show
        when={primary()}
        fallback={
          <section class="card p-5 text-center">
            <div class="font-display text-lg font-bold uppercase tracking-wide text-muted">
              {t("punch.noActiveAccounts")}
            </div>
            <div class="mt-1 text-xs text-muted">{t("punch.hint")}</div>
          </section>
        }
      >
      <section class="grid grid-cols-3 gap-2">
        <PunchButton
          label={primary()?.name ?? "ALLGEMEIN"}
          icon={<Play class="h-7 w-7" />}
          variant="work"
          active={isActive(primary()?.id)}
          disabled={!primary()}
          onClick={() => primary() && request("start", primary()!.id)}
        />
        <PunchButton
          label={t("punch.pause")}
          icon={<Pause class="h-7 w-7" />}
          variant="break"
          active={onBreak()}
          disabled={!canPause()}
          onClick={() => request("pause")}
        />
        <PunchButton
          label={t("punch.stop")}
          icon={<Stop class="h-7 w-7" />}
          variant="stop"
          disabled={!canStop()}
          onClick={() => request("stop")}
        />
      </section>
      </Show>

      <section class="space-y-2">
        <Show when={others().length > 0}>
          <h2 class="text-[11px] font-semibold uppercase tracking-wide text-muted">
            {t("punch.other")}
          </h2>
        </Show>
        <div class="grid gap-2 sm:grid-cols-2">
          <For each={others()}>
            {(account) => (
              <button
                type="button"
                onClick={() => request("start", account.id)}
                class={`flex items-center justify-between gap-3 rounded-xl border-2 px-4 py-4 text-left transition-all active:scale-[0.99] ${
                  isActive(account.id)
                    ? "border-primary bg-primary text-primary-foreground shadow-lg shadow-primary/25"
                    : "border-primary/20 bg-card text-foreground hover:border-primary/40"
                }`}
              >
                <span class="min-w-0 truncate font-display text-lg font-bold uppercase tracking-wide">
                  {account.name}
                </span>
                <Play class="h-5 w-5 shrink-0 opacity-70" />
              </button>
            )}
          </For>

          <button
            type="button"
            onClick={() => {
              setAddError(null);
              setShowAdd(true);
            }}
            class="flex items-center justify-center gap-2 rounded-xl border-2 border-dashed border-line px-4 py-4 font-display text-sm font-bold uppercase tracking-wide text-muted transition-colors hover:border-primary/40 hover:text-primary"
          >
            <span class="text-lg leading-none">＋</span>
            {t("account.add")}
          </button>
        </div>
      </section>

      <section class="card grid grid-cols-3 divide-x divide-line">
        <div class="px-2 py-3 text-center">
          <div class="text-[10px] font-semibold uppercase tracking-wide text-muted">
            {t("day.work")}
          </div>
          <div class="font-display text-xl font-bold tabular-nums">{dur(liveTotals().work)}</div>
        </div>
        <div class="px-2 py-3 text-center">
          <div class="text-[10px] font-semibold uppercase tracking-wide text-muted">
            {t("day.break")}
          </div>
          <div class="font-display text-xl font-bold tabular-nums">
            {dur(liveTotals().break)}
          </div>
        </div>
        <div class="px-2 py-3 text-center">
          <div class="text-[10px] font-semibold uppercase tracking-wide text-muted">
            {t("day.span")}
          </div>
          <div class="font-display text-xl font-bold tabular-nums">{dur(liveTotals().span)}</div>
        </div>
      </section>

      <p class="text-center text-[11px] text-muted">
        {t("day.target")} {dur((today()?.target_minutes ?? 0) * 60_000)}
      </p>

      <Show when={pending()} keyed>
        {(p) => (
          <NoteSheet
            open={true}
            title={`${t("note.title", { account: p.accountName })} · ${timeOfDay(
              p.from,
              locale(),
            )}–${timeOfDay(p.to, locale())}`}
            allowSkip={true}
            onClose={() => setPending(null)}
            onSkip={() => {
              setPending(null);
              void go(p.action, p.accountId);
            }}
            onSave={(note, material) => {
              setPending(null);
              void go(p.action, p.accountId, note, material);
            }}
          />
        )}
      </Show>

      <Sheet
        open={showAdd()}
        title={t("account.add")}
        onClose={() => setShowAdd(false)}
      >
        <form onSubmit={createAccount} class="space-y-3">
          <div>
            <label class="label" for="account-name">
              {t("account.name")}
            </label>
            <input
              id="account-name"
              class="field"
              placeholder={t("account.namePlaceholder")}
              autofocus
              value={newName()}
              onInput={(e) => setNewName(e.currentTarget.value)}
            />
          </div>
          <Show when={addError()}>
            {(message) => <p class="text-sm font-medium text-danger">{message()}</p>}
          </Show>
          <div class="flex gap-2">
            <button type="button" class="btn-ghost flex-1" onClick={() => setShowAdd(false)}>
              {t("common.close")}
            </button>
            <button type="submit" class="btn-primary flex-1" disabled={addBusy()}>
              {addBusy() ? t("common.loading") : t("account.create")}
            </button>
          </div>
        </form>
      </Sheet>
    </div>
  );
}
