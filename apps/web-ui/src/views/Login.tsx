import { Show, createEffect, createSignal, onCleanup } from "solid-js";
import {
  ApiError,
  api,
  assetUrl,
  getServerBase,
  isTauri,
  setServerBase,
  type Branding,
} from "../api";
import { LANGS, lang, setLang, t, type Lang } from "../i18n";
import { applyTheme, signIn } from "../state";

export default function Login() {
  const [server, setServer] = createSignal(getServerBase());
  const [email, setEmail] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [brand, setBrand] = createSignal<Branding | null>(null);
  let brandTimer: number | undefined;

  /**
   * The login screen is unauthenticated, so the brand (name, colours, logo)
   * comes from a public endpoint. It is optional: if the server is unreachable
   * or has no logo, the built-in mark stays.
   */
  async function loadBranding() {
    if (isTauri) {
      const url = server().trim();
      if (!/^https?:\/\//i.test(url)) return;
      setServerBase(url);
    }
    try {
      const value = await api.branding();
      setBrand(value);
      applyTheme(value);
    } catch {
      /* keep the default branding */
    }
  }

  // Fetch once on mount and again shortly after the server address changes.
  createEffect(() => {
    const address = server();
    void address;
    clearTimeout(brandTimer);
    brandTimer = window.setTimeout(() => void loadBranding(), 300);
  });
  onCleanup(() => clearTimeout(brandTimer));

  async function submit(e: Event) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      // The native apps talk to a remote server; the browser uses its origin.
      if (isTauri) setServerBase(server());
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
          <Show
            when={brand()?.logo_url}
            fallback={
              <div class="mb-3 flex min-h-24 flex-col items-center justify-center">
                <div class="mx-auto mb-3 flex h-14 w-14 items-center justify-center rounded-2xl bg-primary text-2xl font-bold text-primary-foreground">
                  D
                </div>
                <h1 class="font-display text-3xl font-bold uppercase tracking-wide">Daywerk</h1>
              </div>
            }
          >
            <div class="mb-3 flex min-h-24 items-center justify-center sm:min-h-28">
              <img
                src={assetUrl(brand()?.logo_url) ?? ""}
                alt={brand()?.name ?? "Daywerk"}
                class="h-24 w-auto max-w-[20rem] object-contain sm:h-28"
              />
            </div>
            <h1 class="sr-only">{brand()?.name ?? "Daywerk"}</h1>
          </Show>
          <p class="mt-1 text-sm text-muted">{t("auth.subtitle")}</p>
        </div>

        <form onSubmit={submit} class="card space-y-3 p-5">
          <Show when={isTauri}>
            <div>
              <label class="label" for="server">
                {t("server.url")}
              </label>
              <input
                id="server"
                type="url"
                class="field"
                inputmode="url"
                autocomplete="url"
                placeholder="https://time.example.org"
                value={server()}
                onInput={(e) => setServer(e.currentTarget.value)}
              />
            </div>
          </Show>
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
