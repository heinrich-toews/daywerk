import { createSignal } from "solid-js";
import { ApiError } from "../api";
import { LANGS, lang, setLang, t, type Lang } from "../i18n";
import { signIn } from "../state";

export default function Login() {
  const [email, setEmail] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function submit(e: Event) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await signIn(email().trim(), password());
    } catch (err) {
      setError(
        err instanceof ApiError && err.status === 401
          ? t("auth.failed")
          : `${t("common.error")}: ${err instanceof Error ? err.message : String(err)}`,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class="flex min-h-screen items-center justify-center px-4 py-10">
      <div class="w-full max-w-sm space-y-5">
        <div class="text-center">
          <div class="mx-auto mb-3 flex h-14 w-14 items-center justify-center rounded-2xl bg-primary text-2xl font-bold text-primary-foreground">
            D
          </div>
          <h1 class="font-display text-3xl font-bold uppercase tracking-wide">Daywerk</h1>
          <p class="mt-1 text-sm text-muted">{t("auth.subtitle")}</p>
        </div>

        <form onSubmit={submit} class="card space-y-3 p-5">
          <div>
            <label class="label" for="email">
              {t("auth.email")}
            </label>
            <input
              id="email"
              type="email"
              class="field"
              autocomplete="username"
              required
              value={email()}
              onInput={(e) => setEmail(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="label" for="password">
              {t("auth.password")}
            </label>
            <input
              id="password"
              type="password"
              class="field"
              autocomplete="current-password"
              required
              value={password()}
              onInput={(e) => setPassword(e.currentTarget.value)}
            />
          </div>

          {error() && (
            <p class="rounded-lg bg-danger/10 px-3 py-2 text-sm font-medium text-danger">
              {error()}
            </p>
          )}

          <button type="submit" class="btn-primary w-full py-3" disabled={busy()}>
            {busy() ? t("common.loading") : t("auth.submit")}
          </button>
        </form>

        <div class="flex justify-center gap-2">
          {LANGS.map((code) => (
            <button
              type="button"
              class={`rounded-full px-3 py-1 text-xs font-semibold uppercase ${
                lang() === code ? "bg-primary text-primary-foreground" : "bg-soft text-primary"
              }`}
              onClick={() => setLang(code as Lang)}
            >
              {code}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
