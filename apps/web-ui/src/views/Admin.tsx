import { For, Show, createEffect, createSignal, onMount } from "solid-js";
import {
  api,
  downloadBackup,
  type Account as TimeAccount,
  type AccountKind,
  type Org,
  type Role,
  type User,
} from "../api";
import { dur } from "../format";
import { t } from "../i18n";
import { me, refresh, signOut } from "../state";
import WorkTimeModelSheet from "../components/WorkTimeModelSheet";

function UserRow(props: {
  user: User;
  orgs: Org[];
  onSaved: (user: User) => void;
  onDeleted: (id: string) => void;
  onModels: (user: User) => void;
}) {
  const [role, setRole] = createSignal<Role>(props.user.role);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const orgName = () => props.orgs.find((o) => o.id === props.user.org_id)?.name ?? "";

  function describe(err: unknown): string {
    const message = err instanceof Error ? err.message : String(err);
    return message.toLowerCase().includes("deactivate") ? t("admin.deleteUserUsed") : message;
  }

  async function save() {
    setBusy(true);
    setError(null);
    try {
      const updated = await api.updateUser(props.user.id, { role: role() });
      props.onSaved(updated);
    } catch (err) {
      setError(describe(err));
    } finally {
      setBusy(false);
    }
  }

  async function toggleActive() {
    setBusy(true);
    setError(null);
    try {
      const updated = await api.updateUser(props.user.id, { active: !props.user.active });
      props.onSaved(updated);
    } catch (err) {
      setError(describe(err));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!window.confirm(`${t("admin.delete")}: ${props.user.name}?`)) return;
    setBusy(true);
    setError(null);
    try {
      await api.deleteUser(props.user.id);
      props.onDeleted(props.user.id);
    } catch (err) {
      setError(describe(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class="border-b border-line py-2 last:border-0">
      <div class="flex flex-wrap items-center gap-2">
      <div class="min-w-0 flex-1">
        <div class="truncate text-sm font-semibold">
          {props.user.name}
          <Show when={!props.user.active}>
            <span class="ml-2 rounded bg-line px-1.5 py-0.5 text-[10px] font-semibold uppercase text-muted">
              {t("admin.inactive")}
            </span>
          </Show>
        </div>
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

      <button
        type="button"
        class="btn-ghost px-3 py-1.5 text-sm"
        onClick={() => props.onModels(props.user)}
      >
        {t("model.open")}
      </button>

      <button type="button" class="btn-ghost px-3 py-1.5 text-sm" disabled={busy()} onClick={() => void save()}>
        {t("settings.save")}
      </button>

      <button
        type="button"
        class="btn-ghost px-3 py-1.5 text-sm"
        disabled={busy()}
        onClick={() => void toggleActive()}
      >
        {props.user.active ? t("admin.deactivate") : t("admin.activate")}
      </button>

      <button
        type="button"
        class="rounded-lg border border-danger/40 px-3 py-1.5 text-sm font-semibold text-danger hover:bg-danger/10"
        disabled={busy()}
        onClick={() => void remove()}
      >
        {t("admin.delete")}
      </button>
      </div>

      <Show when={error()}>
        {(message) => <p class="mt-1 text-xs font-medium text-danger">{message()}</p>}
      </Show>
    </div>
  );
}

export default function Admin() {
  const org = () => me()!.org;

  const [name, setName] = createSignal(org().name);
  const [colorPrimary, setColorPrimary] = createSignal(org().color_primary);
  const [colorAccent, setColorAccent] = createSignal(org().color_accent);
  const [timezone, setTimezone] = createSignal(org().timezone);
  const [weeklyHours, setWeeklyHours] = createSignal(String(org().default_weekly_hours));
  const [workdays, setWorkdays] = createSignal(String(org().default_workdays_per_week));
  const [orgNotice, setOrgNotice] = createSignal<string | null>(null);
  const [editOrgId, setEditOrgId] = createSignal(org().id);
  const [newOrgName, setNewOrgName] = createSignal("");
  const [newOrgSlug, setNewOrgSlug] = createSignal("");

  const [accounts, setAccounts] = createSignal<TimeAccount[]>([]);
  const [accountError, setAccountError] = createSignal<string | null>(null);
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
  const [modelUser, setModelUser] = createSignal<User | null>(null);
  const [importNotice, setImportNotice] = createSignal<string | null>(null);

  async function loadAccounts() {
    try {
      setAccounts(await api.accounts(editOrgId()));
    } catch {
      /* ignore */
    }
  }

  // Accounts belong to the organisation selected above.
  createEffect(() => {
    const id = editOrgId();
    void id;
    void loadAccounts();
  });

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
    setWeeklyHours(String(found.default_weekly_hours));
    setWorkdays(String(found.default_workdays_per_week));
  }

  async function saveOrg(e: Event) {
    e.preventDefault();
    try {
      await api.updateOrgById(editOrgId(), {
        name: name(),
        color_primary: colorPrimary(),
        color_accent: colorAccent(),
        timezone: timezone(),
        default_weekly_hours: Number(weeklyHours().replace(",", ".")),
        default_workdays_per_week: Number(workdays()),
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
    setAccountError(null);
    try {
      await api.createAccount({
        name: accountName().trim(),
        kind: accountKind(),
        sort_order: accountKind() === "break" ? 10 : 100,
        org_id: editOrgId(),
      });
      setAccountName("");
      await loadAccounts();
      if (editOrgId() === org().id) await refresh();
    } catch (err) {
      setAccountError(err instanceof Error ? err.message : String(err));
    }
  }

  async function setArchived(account: TimeAccount, archived: boolean) {
    setAccountError(null);
    try {
      await api.updateAccount(account.id, { archived });
      await loadAccounts();
      if (editOrgId() === org().id) await refresh();
    } catch (err) {
      setAccountError(err instanceof Error ? err.message : String(err));
    }
  }

  async function removeAccount(account: TimeAccount) {
    if (!window.confirm(`${t("admin.delete")}: ${account.name}?`)) return;
    setAccountError(null);
    try {
      await api.deleteAccount(account.id);
      await loadAccounts();
      if (editOrgId() === org().id) await refresh();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setAccountError(
        message.toLowerCase().includes("entries") ? t("admin.deleteUsed") : message,
      );
    }
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
            <label class="label" for="org-hours">
              {t("model.weeklyHours")}
            </label>
            <input
              id="org-hours"
              type="number"
              min="0.25"
              max="80"
              step="0.25"
              class="field"
              value={weeklyHours()}
              onInput={(e) => setWeeklyHours(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="label" for="org-days">
              {t("model.workdays")}
            </label>
            <input
              id="org-days"
              type="number"
              min="1"
              max="7"
              step="1"
              class="field"
              value={workdays()}
              onInput={(e) => setWorkdays(e.currentTarget.value)}
            />
          </div>
          <p class="text-xs text-muted">
            {t("model.daily")}:{" "}
            <b class="text-foreground tabular-nums">
              {dur(
                Math.round(
                  (Number(weeklyHours().replace(",", ".")) * 60) /
                    Math.max(1, Number(workdays())),
                ) * 60_000,
              )}
            </b>
          </p>
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
                    {account.archived ? ` · ${t("admin.archived")}` : ""}
                  </div>
                </div>
                <Show when={!account.is_primary && account.kind !== "break"}>
                  <Show when={account.archived}>
                    <button
                      type="button"
                      class="rounded-lg border border-line px-3 py-1.5 text-xs font-semibold text-muted hover:text-primary"
                      onClick={() => void setArchived(account, false)}
                    >
                      {t("admin.restore")}
                    </button>
                  </Show>
                  <Show when={!account.archived}>
                    <button
                      type="button"
                      class="rounded-lg border border-line px-3 py-1.5 text-xs font-semibold text-muted hover:text-primary"
                      onClick={() => void setArchived(account, true)}
                    >
                      {t("admin.archive")}
                    </button>
                  </Show>
                  <button
                    type="button"
                    class="rounded-lg border border-danger/40 px-3 py-1.5 text-xs font-semibold text-danger hover:bg-danger/10"
                    onClick={() => void removeAccount(account)}
                  >
                    {t("admin.delete")}
                  </button>
                </Show>
              </div>
            )}
          </For>
        </div>

        <Show when={accountError()}>
          {(message) => <p class="text-sm font-medium text-danger">{message()}</p>}
        </Show>

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
                onDeleted={(id) => setUsers((list) => list.filter((u) => u.id !== id))}
                onModels={setModelUser}
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

      <Show when={modelUser()} keyed>
        {(user) => <WorkTimeModelSheet user={user} onClose={() => setModelUser(null)} />}
      </Show>

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
