import { For, Show, createSignal, onMount } from "solid-js";
import {
  api,
  downloadBackup,
  type Account as TimeAccount,
  type AccountKind,
  type Org,
  type Role,
  type User,
} from "../api";
import { t } from "../i18n";
import { me, refresh, signOut } from "../state";

function UserRow(props: { user: User; orgs: Org[]; onSaved: (user: User) => void }) {
  const [role, setRole] = createSignal<Role>(props.user.role);
  const [target, setTarget] = createSignal(
    props.user.daily_target_minutes ? String(props.user.daily_target_minutes) : "",
  );
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal(false);

  const orgName = () => props.orgs.find((o) => o.id === props.user.org_id)?.name ?? "";

  async function save() {
    setBusy(true);
    setError(false);
    try {
      const updated = await api.updateUser(props.user.id, {
        role: role(),
        daily_target_minutes: target() === "" ? 0 : Number(target()),
      });
      props.onSaved(updated);
    } catch {
      setError(true);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class="flex flex-wrap items-center gap-2 border-b border-line py-2 last:border-0">
      <div class="min-w-0 flex-1">
        <div class="truncate text-sm font-semibold">{props.user.name}</div>
        <div class="truncate text-xs text-muted">
          {props.user.email}
          <Show when={orgName()}>
            <span class="ml-1">· {orgName()}</span>
          </Show>
        </div>
      </div>

      <select
        class="rounded-lg border border-line bg-card px-2 py-1.5 text-sm"
        value={role()}
        onChange={(e) => setRole(e.currentTarget.value as Role)}
      >
        <option value="employee">{t("admin.role.employee")}</option>
        <option value="office">{t("admin.role.office")}</option>
        <option value="admin">{t("admin.role.admin")}</option>
      </select>

      <input
        class="w-20 rounded-lg border border-line bg-card px-2 py-1.5 text-sm tabular-nums"
        type="number"
        min="0"
        step="15"
        placeholder={t("admin.targetUser")}
        value={target()}
        onInput={(e) => setTarget(e.currentTarget.value)}
      />

      <button type="button" class="btn-ghost px-3 py-1.5 text-sm" disabled={busy()} onClick={() => void save()}>
        {error() ? "!" : t("settings.save")}
      </button>
    </div>
  );
}

export default function Admin() {
  const org = () => me()!.org;

  const [name, setName] = createSignal(org().name);
  const [colorPrimary, setColorPrimary] = createSignal(org().color_primary);
  const [colorAccent, setColorAccent] = createSignal(org().color_accent);
  const [timezone, setTimezone] = createSignal(org().timezone);
  const [target, setTarget] = createSignal(String(org().daily_target_minutes));
  const [orgNotice, setOrgNotice] = createSignal<string | null>(null);
  const [editOrgId, setEditOrgId] = createSignal(org().id);
  const [newOrgName, setNewOrgName] = createSignal("");
  const [newOrgSlug, setNewOrgSlug] = createSignal("");

  const [accounts, setAccounts] = createSignal<TimeAccount[]>([]);
  const [accountName, setAccountName] = createSignal("");
  const [accountKind, setAccountKind] = createSignal<AccountKind>("work");

  const [users, setUsers] = createSignal<User[]>([]);
  const [orgs, setOrgs] = createSignal<Org[]>([]);
  const [newEmail, setNewEmail] = createSignal("");
  const [newName, setNewName] = createSignal("");
  const [newPassword, setNewPassword] = createSignal("");
  const [newRole, setNewRole] = createSignal<Role>("employee");
  const [newOrgId, setNewOrgId] = createSignal("");
  const [userError, setUserError] = createSignal<string | null>(null);
  const [importNotice, setImportNotice] = createSignal<string | null>(null);

  async function loadAccounts() {
    try {
      setAccounts(await api.accounts());
    } catch {
      /* ignore */
    }
  }

  async function loadUsers() {
    try {
      setUsers(await api.users());
      setOrgs(await api.orgs());
    } catch {
      /* ignore */
    }
  }

  onMount(() => {
    void loadAccounts();
    void loadUsers();
  });

  function selectOrg(id: string) {
    setEditOrgId(id);
    const found = orgs().find((o) => o.id === id);
    if (!found) return;
    setName(found.name);
    setColorPrimary(found.color_primary);
    setColorAccent(found.color_accent);
    setTimezone(found.timezone);
    setTarget(String(found.daily_target_minutes));
  }

  async function saveOrg(e: Event) {
    e.preventDefault();
    try {
      await api.updateOrgById(editOrgId(), {
        name: name(),
        color_primary: colorPrimary(),
        color_accent: colorAccent(),
        timezone: timezone(),
        daily_target_minutes: Number(target()),
      });
      if (editOrgId() === org().id) await refresh();
      setOrgs(await api.orgs());
      setOrgNotice(t("settings.saved"));
    } catch (err) {
      setOrgNotice(err instanceof Error ? err.message : String(err));
    }
  }

  async function addOrg(e: Event) {
    e.preventDefault();
    if (!newOrgName().trim() || !newOrgSlug().trim()) return;
    try {
      const created = await api.createOrg({
        name: newOrgName().trim(),
        slug: newOrgSlug().trim().toLowerCase(),
      });
      setNewOrgName("");
      setNewOrgSlug("");
      setOrgs(await api.orgs());
      selectOrg(created.id);
      setOrgNotice(t("settings.saved"));
    } catch (err) {
      setOrgNotice(err instanceof Error ? err.message : String(err));
    }
  }

  async function addAccount(e: Event) {
    e.preventDefault();
    if (!accountName().trim()) return;
    await api.createAccount({
      name: accountName().trim(),
      kind: accountKind(),
      sort_order: accountKind() === "break" ? 10 : 100,
    });
    setAccountName("");
    await loadAccounts();
    await refresh();
  }

  async function archive(account: TimeAccount) {
    await api.updateAccount(account.id, { archived: true });
    await loadAccounts();
    await refresh();
  }

  async function addUser(e: Event) {
    e.preventDefault();
    setUserError(null);
    try {
      await api.createUser({
        email: newEmail().trim(),
        name: newName().trim(),
        password: newPassword(),
        role: newRole(),
        ...(newOrgId() ? { org_id: newOrgId() } : {}),
      });
      setNewEmail("");
      setNewName("");
      setNewPassword("");
      await loadUsers();
    } catch (err) {
      setUserError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onImport(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try {
      const parsed: unknown = JSON.parse(await file.text());
      await api.importBackup(parsed);
      setImportNotice(t("settings.imported"));
      // The import replaces all sessions, so the current token is invalid.
      await signOut();
    } catch (err) {
      setImportNotice(err instanceof Error ? err.message : String(err));
    } finally {
      input.value = "";
    }
  }

  return (
    <div class="space-y-4">
      <h1 class="font-display text-2xl font-bold uppercase tracking-wide">{t("admin.title")}</h1>

      <form onSubmit={saveOrg} class="card space-y-3 p-4">
        <h2 class="font-display text-lg font-bold uppercase">{t("admin.org")}</h2>
        <Show when={orgs().length > 0}>
          <div>
            <label class="label" for="org-select">
              {t("team.org")}
            </label>
            <select
              id="org-select"
              class="field"
              value={editOrgId()}
              onChange={(e) => selectOrg(e.currentTarget.value)}
            >
              <For each={orgs()}>{(o) => <option value={o.id}>{o.name}</option>}</For>
            </select>
          </div>
        </Show>
        <div>
          <label class="label" for="org-name">
            {t("admin.orgName")}
          </label>
          <input
            id="org-name"
            class="field"
            value={name()}
            onInput={(e) => setName(e.currentTarget.value)}
          />
        </div>
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="label" for="org-primary">
              {t("admin.colorPrimary")}
            </label>
            <input
              id="org-primary"
              type="color"
              class="h-10 w-full rounded-lg border border-line bg-card"
              value={colorPrimary()}
              onInput={(e) => setColorPrimary(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="label" for="org-accent">
              {t("admin.colorAccent")}
            </label>
            <input
              id="org-accent"
              type="color"
              class="h-10 w-full rounded-lg border border-line bg-card"
              value={colorAccent()}
              onInput={(e) => setColorAccent(e.currentTarget.value)}
            />
          </div>
        </div>
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="label" for="org-tz">
              {t("admin.timezone")}
            </label>
            <input
              id="org-tz"
              class="field"
              value={timezone()}
              onInput={(e) => setTimezone(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="label" for="org-target">
              {t("admin.target")}
            </label>
            <input
              id="org-target"
              type="number"
              min="0"
              step="15"
              class="field"
              value={target()}
              onInput={(e) => setTarget(e.currentTarget.value)}
            />
          </div>
        </div>
        <Show when={orgNotice()}>
          {(message) => <p class="text-sm font-medium text-ok">{message()}</p>}
        </Show>
        <button type="submit" class="btn-primary w-full">
          {t("settings.save")}
        </button>
      </form>

      <form onSubmit={addOrg} class="card space-y-3 p-4">
        <h2 class="font-display text-lg font-bold uppercase">{t("admin.newOrg")}</h2>
        <div class="grid grid-cols-2 gap-2">
          <input
            class="field"
            placeholder={t("admin.orgName")}
            required
            value={newOrgName()}
            onInput={(e) => setNewOrgName(e.currentTarget.value)}
          />
          <input
            class="field"
            placeholder="slug"
            required
            value={newOrgSlug()}
            onInput={(e) => setNewOrgSlug(e.currentTarget.value)}
          />
        </div>
        <button type="submit" class="btn-ghost w-full">
          {t("common.add")}
        </button>
      </form>

      <section class="card space-y-3 p-4">
        <h2 class="font-display text-lg font-bold uppercase">{t("admin.accounts")}</h2>
        <div class="divide-y divide-line">
          <For each={accounts()}>
            {(account) => (
              <div class="flex items-center gap-2 py-2">
                <div class="min-w-0 flex-1">
                  <div class="truncate text-sm font-semibold">{account.name}</div>
                  <div class="text-xs text-muted">
                    {account.kind === "break" ? t("day.break") : t("day.work")}
                    {account.is_primary ? ` · ${t("admin.primary")}` : ""}
                    {account.archived ? " · ✕" : ""}
                  </div>
                </div>
                <Show when={!account.is_primary && account.kind !== "break" && !account.archived}>
                  <button
                    type="button"
                    class="rounded-lg border border-line px-3 py-1.5 text-xs font-semibold text-muted hover:text-danger"
                    onClick={() => void archive(account)}
                  >
                    {t("admin.archive")}
                  </button>
                </Show>
              </div>
            )}
          </For>
        </div>

        <form onSubmit={addAccount} class="flex flex-wrap items-end gap-2">
          <div class="min-w-40 flex-1">
            <label class="label" for="acc-name">
              {t("admin.addAccount")}
            </label>
            <input
              id="acc-name"
              class="field"
              placeholder={t("admin.accountPlaceholder")}
              value={accountName()}
              onInput={(e) => setAccountName(e.currentTarget.value)}
            />
          </div>
          <select
            class="h-11 rounded-lg border border-line bg-card px-2"
            value={accountKind()}
            onChange={(e) => setAccountKind(e.currentTarget.value as AccountKind)}
          >
            <option value="work">{t("day.work")}</option>
            <option value="break">{t("day.break")}</option>
          </select>
          <button type="submit" class="btn-primary h-11">
            {t("common.add")}
          </button>
        </form>
      </section>

      <section class="card space-y-3 p-4">
        <h2 class="font-display text-lg font-bold uppercase">{t("admin.users")}</h2>

        <div>
          <For each={users()}>
            {(user) => (
              <UserRow
                user={user}
                orgs={orgs()}
                onSaved={(updated) =>
                  setUsers((list) => list.map((u) => (u.id === updated.id ? updated : u)))
                }
              />
            )}
          </For>
        </div>

        <form onSubmit={addUser} class="space-y-2 border-t border-line pt-3">
          <h3 class="text-xs font-semibold uppercase tracking-wide text-muted">
            {t("admin.addUser")}
          </h3>
          <div class="grid grid-cols-2 gap-2">
            <input
              class="field"
              type="email"
              placeholder={t("admin.email")}
              required
              value={newEmail()}
              onInput={(e) => setNewEmail(e.currentTarget.value)}
            />
            <input
              class="field"
              placeholder={t("settings.name")}
              required
              value={newName()}
              onInput={(e) => setNewName(e.currentTarget.value)}
            />
            <input
              class="field"
              type="password"
              placeholder={t("settings.password")}
              required
              value={newPassword()}
              onInput={(e) => setNewPassword(e.currentTarget.value)}
            />
            <select
              class="field"
              value={newRole()}
              onChange={(e) => setNewRole(e.currentTarget.value as Role)}
            >
              <option value="employee">{t("admin.role.employee")}</option>
              <option value="office">{t("admin.role.office")}</option>
              <option value="admin">{t("admin.role.admin")}</option>
            </select>
            <Show when={orgs().length > 1}>
              <select
                class="field col-span-2"
                value={newOrgId()}
                onChange={(e) => setNewOrgId(e.currentTarget.value)}
              >
                <option value="">{org().name}</option>
                <For each={orgs()}>{(o) => <option value={o.id}>{o.name}</option>}</For>
              </select>
            </Show>
          </div>
          <Show when={userError()}>
            {(message) => <p class="text-sm text-danger">{message()}</p>}
          </Show>
          <button type="submit" class="btn-primary w-full">
            {t("common.add")}
          </button>
        </form>
      </section>

      <section class="card space-y-3 p-4">
        <h2 class="font-display text-lg font-bold uppercase">{t("settings.backup")}</h2>
        <div class="flex flex-wrap gap-2">
          <button type="button" class="btn-ghost flex-1" onClick={() => void downloadBackup()}>
            {t("settings.download")}
          </button>
          <label class="btn-ghost flex-1 cursor-pointer text-center">
            {t("settings.import")}
            <input type="file" accept="application/json" class="hidden" onChange={onImport} />
          </label>
        </div>
        <p class="text-xs text-muted">{t("settings.importWarn")}</p>
        <Show when={importNotice()}>
          {(message) => <p class="text-sm font-medium text-ok">{message()}</p>}
        </Show>
      </section>
    </div>
  );
}
