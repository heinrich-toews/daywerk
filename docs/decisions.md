# Decisions

Recorded while building the first version. The numbers refer to the plan that was
agreed before implementation.

| # | Decision | Result |
|---|---|---|
| D1 | Stack | Rust workspace + SolidJS + Tailwind v4, modelled on OpenChordBook; Tauri v2 for native clients |
| D2 | Role scope | `admin` global, `office` limited to its own organisation, `employee` limited to its own data |
| D3 | "Zeitkonto" | **A real hours balance from the start** — daily target per organisation, optional per-user override, manual adjustments, balance per period and total |
| D4 | Stop while paused | Delete the current break segment; the last work segment ends when the break began |
| D5 | Midnight | A shift may cross midnight; the day belongs to its start date |
| D6 | Corrections | Employees: own day only (interface follows); office/admin: any, with audit log entries |
| D7 | Offline | Required for the native clients; punch is server-authoritative and will be replayed from an outbox |
| D8 | Licence | AGPL-3.0-only, SPDX headers, DCO sign-off |
| D9 | Sign-up | Admin-created accounts only, no self-registration |
| D10 | Import | **JSON only** — a single self-contained backup/restore file |
| D11 | Language | **German and English**, stored per user and switchable in the profile |
| D12 | HTTP | `axum` instead of `tiny_http` — typed routing and extractors for a growing REST surface |

## Consequences worth knowing

**Balance formula.** `worked − (target × days with entries) + adjustments`. Counting
only days with entries means a day off does not create a negative balance. If a strict
"every working day counts" model is wanted later, the day count has to come from a
calendar, not from `workdays` rows.

**Server-authoritative punching.** The client sends an intention (`start`/`pause`/`stop`),
never a computed segment list. The server derives the mutations and applies them in one
transaction. This removes most conflict cases before there are native clients.

**One open segment.** Enforced by a partial unique index
`ON segments(user_id) WHERE ended_at IS NULL`, so even a bug in application code cannot
produce two running timers.

**Tenant isolation.** SQLite has no row-level security. Every storage function that
returns tenant data takes an organisation id, and the HTTP layer derives that id from
the authenticated user rather than from the request.

**Pause is data, not a flag.** `PAUSE` is an ordinary account row with `kind = 'break'`.
The state machine special-cases it only where the requirements demand it (stop while
paused). A unique index guarantees exactly one break account per organisation.

## Naming

All identifiers are English: Rust types and variants (`Role::Employee`,
`Role::Office`), database values (`employee`, `office`, `admin`), API payloads
and TypeScript types. German and English only appear as *translations* in
`apps/web-ui/src/i18n.ts`, and as data the user can edit — notably the seeded
account names `ALLGEMEIN` and `PAUSE`, which the specification requires.

**Pre-1.0:** the schema is not stable yet. After a schema change, delete
`data/daywerk.db` and let the server recreate it.
