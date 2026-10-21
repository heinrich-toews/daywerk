/** Duration formatting shared by all views. */

export function dur(ms: number): string {
  const neg = ms < 0;
  const total = Math.round(Math.abs(ms) / 60_000);
  const text = `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
  return neg ? `-${text}` : text;
}

export function durSeconds(ms: number): string {
  const neg = ms < 0;
  const abs = Math.floor(Math.abs(ms) / 1000);
  const h = Math.floor(abs / 3600);
  const m = Math.floor((abs % 3600) / 60);
  const s = abs % 60;
  const text = `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return neg ? `-${text}` : text;
}

export function clockAt(ms: number, locale: string): string {
  return new Date(ms).toLocaleTimeString(locale, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function timeOfDay(ms: number, locale: string): string {
  return new Date(ms).toLocaleTimeString(locale, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function longDate(dateIso: string, locale: string): string {
  // Date-only strings must be parsed as local time, not as UTC midnight.
  const date = new Date(`${dateIso}T00:00:00`);
  return date.toLocaleDateString(locale, {
    weekday: "short",
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
  });
}

export function todayIso(): string {
  const now = new Date();
  const local = new Date(now.getTime() - now.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 10);
}

export function monthStartIso(): string {
  return `${todayIso().slice(0, 7)}-01`;
}

export function balanceClass(ms: number): string {
  if (ms > 0) return "text-ok";
  if (ms < 0) return "text-danger";
  return "text-muted";
}
