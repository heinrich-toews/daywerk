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

## Notes on transitions

Notes and materials belong to the segment that is **closed**, not to the one that
starts. The punch request carries them and the server applies them in the same
transaction that closes the segment, so a note can never end up on the wrong
entry. Breaks are never annotated: stopping while paused deletes the break, and
the work segment already carries the notes it got when the break began.

## Deleting time accounts

An account with booked segments is never deleted — that would rewrite history.
The API refuses with `409` and the UI offers archiving instead. The primary
account and the break account cannot be deleted either; they are structural.
Archived accounts keep their names free for reuse, and restoring one that
collides with a new active name is refused by a partial unique index.

## Removing employees

Same rule as time accounts: an employee with booked time is never deleted,
because the record has to survive. The API answers `409` and the office
deactivates instead. A deactivated employee cannot log in (the session lookup
requires `active = 1`) but stays in the team report and in exports, so their
hours and balance remain readable. Employees without any entries are deleted
for real, and the last active administrator is protected from both deletion and
deactivation.

## Working time models

Contracts are expressed in **hours per week**, not minutes per day. Each employee
has an append-only list of `work_time_models` rows (`valid_from`, `weekly_hours`,
`workdays_per_week`). The daily target is derived:

```
daily_target_minutes = round(weekly_hours * 60 / workdays_per_week)
```

Why append-only instead of a `users.weekly_hours` column: a contract change must
not rewrite history. The model in force on a day is the newest row starting on or
before that day, so a change from 40 h to 30 h on 1 November leaves every October
balance untouched. There is no `valid_to` column — validity ends where the next
row begins, which makes gaps and overlaps structurally impossible.

Balance is therefore computed per day, not per period. Only days with entries
count (a holiday week neither adds nor subtracts), and manual
`balance_adjustments` still apply on top.

Retroactive `valid_from` dates are allowed (contract corrections) and audited;
they intentionally recalculate the past. Editing an existing row is reserved for
typos.

## Archiving time accounts

Time accounts are archived, not deleted: the punch screen lists only active
accounts, so the buttons stay limited to current projects, while history,
statistics, search and exports keep resolving the archived names. Archiving is
allowed for office and admin (office scoped to its own organisation).

Two accounts are structural and cannot be archived: `ALLGEMEIN` (the primary
work account, always the first button) and `PAUSE` (the single break account).

An account archived while a timer is running **stays visible on the punch screen
until that segment is stopped or switched**; the day payload adds the running
account back even though it is archived. After the segment closes it disappears
on the next refresh. Archiving never interrupts a running timer.

Deleting an account is always possible — except the two structural ones — and
it destroys the account's bookings. The segments go, days that are left without
any segment go too (unless they carry a note, which is history of its own), and
the audit log records the name, the number of entries and the duration. The UI
asks first and shows exactly what would be lost, from
`GET /api/accounts/{id}/usage`; archiving stays the non-destructive option.

## Task management

The app has two worlds, chosen in the header: **Zeiterfassung** (the punch clock)
and **Aufgaben** (projects and tasks). They are deliberately separate: the punch
world knows nothing about tasks, the task world writes no punch data.

The only bridge is `projects.account_id`. A project may be linked to a time
account, and then reports the hours booked on that account — read-only, derived
from segments. This keeps a single timer and a single source of truth for worked
time.

Task effort is **entered manually**, not measured. Every entry is a row in
`task_time_entries` (minutes, date, note, author), and the task shows the sum.
There is no task timer and no `segments.task_id`.

Deleting a project is a hard delete with cascade: tasks, subtasks, comments and
time entries are removed, and the link to the time account is released. The time
account and its booked segments survive. The UI asks for confirmation with the
exact counts.

Projects are written by office and admin only; tasks and subtasks are open to
everyone in the organisation, because the board is shared. Editing a task is
allowed for its creator, its assignee, office and admin.

## Phone layout

The layout is responsive without a separate mobile build. On screens narrower
than Tailwind's `sm` (640 px) the world's tabs move from the header into a
**bottom navigation bar**; on wider screens the header bar stays as it was.

The reason is reach, not taste: on a phone the top bar is only comfortable for
one hand if the device is held high, and the tab labels at `text-[11px]` with
`py-1.5` were under the 48 px touch-target minimum. The bottom items are
full-width thirds/fifths with a 64 px height, an icon plus its label, and a
2 px active indicator at the top edge — they sit exactly where a right thumb
rests. The header keeps the organisation, user, world switch, settings and
logout.

The content area reserves the bar's height (`pb-24` below `sm`), so lists and
the last card never scroll underneath it. Views without tabs (settings, admin)
hide the bar on both layouts.

## Branding: colours and logo

An organisation carries a name, two colours and a logo. The colours are CSS
variables (`--dw-brand`, `--dw-soft`) applied on boot *and* on the login screen;
the logo is a binary blob in `org_logos`, uploaded by an admin through
`PUT /api/orgs/{id}/logo` and served from `GET /api/orgs/{id}/logo?v=<upload>`.

Storing the bytes in SQLite (rather than a path on disk) keeps the deployment a
single writable directory: no upload folder to back up, and no way to point the
server at an arbitrary file. Images are capped at 512 KB and checked by magic
bytes (PNG, JPEG, WebP). SVG is refused: it can carry script, and the logo is
served from the API origin.

`organizations.logo_path` keeps the versioned URL, so every existing `Org`
response already carries the logo and the client needs no extra lookup. The
`?v=` suffix lets the browser cache the image indefinitely while a replacement
is visible immediately.

The login screen is unauthenticated, so it reads the brand from
`GET /api/branding` (name, colours, logo of the oldest organisation). That is
also why the brand colours now appear before sign-in. A multi-tenant instance
shows the oldest organisation on the login page; one deployment per customer is
the intended shape.

The logo travels inside the JSON backup as base64, so a restore keeps the
branding. Older backups without the `org_logos` key still import.
