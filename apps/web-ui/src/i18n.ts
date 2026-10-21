import { createSignal } from "solid-js";

export type Lang = "de" | "en";
export const LANGS: Lang[] = ["de", "en"];

const de = {
  "app.tagline": "Zeiten stempeln, klar und einfach.",
  "nav.today": "Heute",
  "nav.history": "Verlauf",
  "nav.stats": "Statistik",
  "nav.team": "Team",
  "nav.settings": "Einstellungen",
  "nav.admin": "Verwaltung",

  "auth.title": "Anmelden",
  "auth.subtitle": "Melde dich an, um deine Arbeitszeit zu stempeln.",
  "auth.email": "E-Mail",
  "auth.password": "Passwort",
  "auth.submit": "Anmelden",
  "auth.failed": "Anmeldung fehlgeschlagen.",

  "punch.pause": "PAUSE",
  "punch.stop": "STOP",
  "punch.other": "Weitere Zeitkonten",
  "punch.activeSince": "Läuft seit {time}",
  "punch.idle": "Bereit zum Einstempeln",
  "punch.closed": "Tag abgeschlossen",
  "punch.noAccounts": "Noch keine Zeitkonten angelegt.",
  "punch.hint": "Tippe ein Zeitkonto an, um die Messung zu starten.",

  "day.work": "Arbeitszeit",
  "day.break": "Pause",
  "day.span": "Anwesenheit",
  "day.target": "Soll",

  "balance.month": "Saldo Monat",
  "balance.total": "Saldo gesamt",

  "history.title": "Verlauf",
  "history.from": "Von",
  "history.to": "Bis",
  "history.empty": "Keine Einträge in diesem Zeitraum.",

  "stats.title": "Statistik",
  "stats.period": "Zeitraum",
  "stats.perAccount": "Nach Zeitkonto",
  "stats.perDay": "Nach Tag",
  "stats.days": "Arbeitstage",
  "stats.worked": "Gearbeitet",

  "team.title": "Team",
  "team.worker": "Mitarbeiter",
  "team.org": "Organisation",
  "team.balancePeriod": "Saldo Zeitraum",
  "team.balanceTotal": "Saldo gesamt",
  "team.worked": "Gearbeitet",
  "team.empty": "Keine Mitarbeiter gefunden.",
  "team.allOrgs": "Alle Organisationen",

  "settings.title": "Einstellungen",
  "settings.profile": "Profil",
  "settings.name": "Name",
  "settings.language": "Sprache",
  "settings.password": "Neues Passwort",
  "settings.passwordHint": "Leer lassen, um das Passwort nicht zu ändern.",
  "settings.save": "Speichern",
  "settings.saved": "Gespeichert.",
  "settings.logout": "Abmelden",
  "settings.backup": "Sicherung",
  "settings.download": "JSON herunterladen",
  "settings.import": "JSON importieren",
  "settings.importWarn": "Der Import ersetzt alle Daten und meldet dich ab.",
  "settings.imported": "Import abgeschlossen.",

  "admin.title": "Verwaltung",
  "admin.org": "Organisation",
  "admin.orgName": "Name",
  "admin.colorPrimary": "Farbe",
  "admin.colorAccent": "Akzent",
  "admin.timezone": "Zeitzone",
  "admin.target": "Tagessoll (Minuten)",
  "admin.accounts": "Zeitkonten",
  "admin.addAccount": "Zeitkonto anlegen",
  "admin.users": "Mitarbeiter",
  "admin.addUser": "Mitarbeiter anlegen",
  "admin.email": "E-Mail",
  "admin.role": "Rolle",
  "admin.role.allgemein": "Allgemein",
  "admin.role.buero": "Büro",
  "admin.role.admin": "Admin",
  "admin.targetUser": "Soll (Min.)",
  "admin.archive": "Archivieren",

  "common.loading": "Lädt …",
  "common.error": "Fehler",
  "common.add": "Hinzufügen",
  "common.none": "—",
  "common.minutes": "Min.",
  "common.noData": "Keine Daten.",
};

type Key = keyof typeof de;

const en: Record<Key, string> = {
  "app.tagline": "Punch in, clear and simple.",
  "nav.today": "Today",
  "nav.history": "History",
  "nav.stats": "Statistics",
  "nav.team": "Team",
  "nav.settings": "Settings",
  "nav.admin": "Admin",

  "auth.title": "Sign in",
  "auth.subtitle": "Sign in to punch your working time.",
  "auth.email": "Email",
  "auth.password": "Password",
  "auth.submit": "Sign in",
  "auth.failed": "Sign-in failed.",

  "punch.pause": "BREAK",
  "punch.stop": "STOP",
  "punch.other": "Other time accounts",
  "punch.activeSince": "Running since {time}",
  "punch.idle": "Ready to punch in",
  "punch.closed": "Day closed",
  "punch.noAccounts": "No time accounts yet.",
  "punch.hint": "Tap an account to start tracking.",

  "day.work": "Working time",
  "day.break": "Break",
  "day.span": "On site",
  "day.target": "Target",

  "balance.month": "Balance month",
  "balance.total": "Balance total",

  "history.title": "History",
  "history.from": "From",
  "history.to": "To",
  "history.empty": "No entries in this period.",

  "stats.title": "Statistics",
  "stats.period": "Period",
  "stats.perAccount": "By account",
  "stats.perDay": "By day",
  "stats.days": "Working days",
  "stats.worked": "Worked",

  "team.title": "Team",
  "team.worker": "Employee",
  "team.org": "Organisation",
  "team.balancePeriod": "Balance period",
  "team.balanceTotal": "Balance total",
  "team.worked": "Worked",
  "team.empty": "No employees found.",
  "team.allOrgs": "All organisations",

  "settings.title": "Settings",
  "settings.profile": "Profile",
  "settings.name": "Name",
  "settings.language": "Language",
  "settings.password": "New password",
  "settings.passwordHint": "Leave empty to keep the current password.",
  "settings.save": "Save",
  "settings.saved": "Saved.",
  "settings.logout": "Sign out",
  "settings.backup": "Backup",
  "settings.download": "Download JSON",
  "settings.import": "Import JSON",
  "settings.importWarn": "Importing replaces all data and signs you out.",
  "settings.imported": "Import finished.",

  "admin.title": "Administration",
  "admin.org": "Organisation",
  "admin.orgName": "Name",
  "admin.colorPrimary": "Colour",
  "admin.colorAccent": "Accent",
  "admin.timezone": "Timezone",
  "admin.target": "Daily target (minutes)",
  "admin.accounts": "Time accounts",
  "admin.addAccount": "Create time account",
  "admin.users": "Employees",
  "admin.addUser": "Create employee",
  "admin.email": "Email",
  "admin.role": "Role",
  "admin.role.allgemein": "General",
  "admin.role.buero": "Office",
  "admin.role.admin": "Admin",
  "admin.targetUser": "Target (min)",
  "admin.archive": "Archive",

  "common.loading": "Loading …",
  "common.error": "Error",
  "common.add": "Add",
  "common.none": "—",
  "common.minutes": "min",
  "common.noData": "No data.",
};

const dictionaries: Record<Lang, Record<Key, string>> = { de, en };

const [lang, setLangSignal] = createSignal<Lang>("de");
export { lang };

export function setLang(next: Lang) {
  setLangSignal(next);
  if (typeof document !== "undefined") document.documentElement.lang = next;
}

export function t(key: Key, vars?: Record<string, string | number>): string {
  let text = dictionaries[lang()][key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
  }
  return text;
}

export function locale(): string {
  return lang() === "de" ? "de-DE" : "en-GB";
}
