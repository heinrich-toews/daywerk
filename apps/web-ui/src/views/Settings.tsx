import { For, Show, createSignal } from "solid-js";
import { api, getServerBase, isTauri, setServerBase } from "../api";
import { LANGS, lang, t, type Lang } from "../i18n";
import { changeLocale, me, refresh, roleAtLeast, setView } from "../state";

export default function Settings() {
  const [name, setName] = createSignal(me()?.user.name ?? "");
  const [server, setServer] = createSignal(getServerBase());
  const [password, setPassword] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [notice, setNotice] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  async function save(e: Event) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await api.updateProfile({
        name: name(),
        ...(password() ? { password: password() } : {}),
      });
      setPassword("");
      await refresh();
      setNotice(t("settings.saved"));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  function saveServer(e: Event) {
    e.preventDefault();
    setServerBase(server());
    window.location.reload();
  }

  async function pickLanguage(code: Lang) {
    try {
      await changeLocale(code);
      setNotice(t("settings.saved"));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <div class="space-y-4">
      <h1 class="font-display text-2xl font-bold uppercase tracking-wide">
        {t("settings.title")}
      </h1>

      <form onSubmit={save} class="card space-y-3 p-4">
        <h2 class="font-display text-lg font-bold uppercase">{t("settings.profile")}</h2>

        <div>
          <label class="label" for="profile-name">
            {t("settings.name")}
          </label>
          <input
            id="profile-name"
            class="field"
            value={name()}
            onInput={(e) => setName(e.currentTarget.value)}
          />
        </div>

        <div>
          <span class="label">{t("settings.language")}</span>
          <div class="flex gap-2">
            <For each={LANGS}>
              {(code) => (
                <button
                  type="button"
                  onClick={() => void pickLanguage(code)}
                  class={`rounded-lg px-4 py-2 text-sm font-semibold uppercase ${
                    lang() === code
                      ? "bg-primary text-primary-foreground"
                      : "border border-line bg-card"
                  }`}
                >
                  {code}
                </button>
              )}
            </For>
          </div>
        </div>

        <div>
          <label class="label" for="profile-password">
            {t("settings.password")}
          </label>
          <input
            id="profile-password"
            type="password"
            class="field"
            autocomplete="new-password"
            value={password()}
            onInput={(e) => setPassword(e.currentTarget.value)}
          />
          <p class="mt-1 text-xs text-muted">{t("settings.passwordHint")}</p>
        </div>

        <Show when={notice()}>
          {(message) => <p class="text-sm font-medium text-ok">{message()}</p>}
        </Show>
        <Show when={error()}>
          {(message) => <p class="text-sm font-medium text-danger">{message()}</p>}
        </Show>

        <button type="submit" class="btn-primary w-full" disabled={busy()}>
          {busy() ? t("common.loading") : t("settings.save")}
        </button>
      </form>

      <Show when={isTauri}>
        <form onSubmit={saveServer} class="card space-y-3 p-4">
          <h2 class="font-display text-lg font-bold uppercase">{t("server.title")}</h2>
          <div>
            <label class="label" for="server-url">
              {t("server.url")}
            </label>
            <input
              id="server-url"
              type="url"
              class="field"
              inputmode="url"
              autocomplete="url"
              placeholder="https://time.example.org"
              value={server()}
              onInput={(e) => setServer(e.currentTarget.value)}
            />
            <p class="mt-1 text-xs text-muted">{t("server.hint")}</p>
          </div>
          <button type="submit" class="btn-primary w-full">
            {t("server.save")}
          </button>
        </form>
      </Show>

      <Show when={roleAtLeast("admin")}>
        <button type="button" class="btn-ghost w-full" onClick={() => setView("admin")}>
          {t("nav.admin")}
        </button>
      </Show>
    </div>
  );
}
