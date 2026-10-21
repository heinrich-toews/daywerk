# Daywerk

**A punch clock for small teams — open source, self-hosted, mobile first.**

Employees tap one big button to start working, tap another to switch jobs, and the
office sees the hours and the running balance. No spreadsheets, no paper sheets.

```
ALLGEMEIN  │  PAUSE  │  STOP
───────────┼─────────┼───────
Customer TWx │  Storage  │  …
```

## Why another time tracker?

Because the last one stored *one row per day with typed-in times*. Daywerk records
**segments**: one account is running at any moment, switching closes the previous
segment and opens the next one at the same instant. That makes the data exact,
auditable and impossible to double-book.

## Features

- **One exclusive timer** — starting another time account stops the previous one
- **Default accounts** `ALLGEMEIN` (work) and `PAUSE` (break); anyone can add work accounts
  right on the punch screen, admins add/archive/delete the rest
- **Notes and materials** are captured whenever a work segment is closed — switching jobs,
  pausing or stopping — and shown in the history
- **Stop while paused** deletes the break and ends the work segment when the break began
- **Real hours balance** per worker: `worked − (target × days) + adjustments`
  with an optional per-user target override
- **Three roles** — `employee` (own data), `office` (own organisation), `admin` (all organisations)
- **Multi-tenant** — any number of organisations, each with its own accounts and branding
- **German and English UI**, selectable per user
- **Search** across notes and materials, scoped to the office's organisation
- **XLSX and PDF export** of the timesheet (summary, per day, raw segments with notes)
- **JSON backup / restore** as a single file
- **REST API** for the web UI and native clients (Tauri planned)
- **Branding** — logo path, primary colour and accent colour per organisation

## Stack

| Layer | Choice |
|---|---|
| Domain | Rust (`dw-core`) — pure state machine, no I/O, fully tested |
| Storage | SQLite (`dw-storage`) via `rusqlite`, migrations through `PRAGMA user_version` |
| Server | Rust + `axum` (`dw-server`), bearer tokens, Argon2id passwords |
| Web UI | SolidJS + Tailwind CSS v4 + Vite |
| Native | Tauri v2 (planned, shares the web UI) |

The architecture follows [OpenChordBook](https://github.com/openchordbook/openchordbook):
one self-contained binary, one SQLite file, no external services.

## Quick start

Requirements: Rust ≥ 1.85, Node ≥ 22, pnpm.

```sh
# 1. build the web UI
pnpm install
pnpm build

# 2. build and start the server
cargo build -p dw-server
ADMIN_EMAIL=admin@example.org ADMIN_PASSWORD=change-me \
  DB_PATH=data/daywerk.db WEB_ROOT=apps/web-ui/dist \
  ./target/debug/dw-server
```

Open <http://localhost:9095> and sign in with the credentials above. The first
start creates the organisation, the default accounts and the administrator.

Development, with hot reload:

```sh
cargo run -p dw-server     # API on :9095
pnpm dev                   # UI on :5173, proxies /api to :9095
```

Run the test suite:

```sh
cargo test --workspace
python3 scripts/api-smoke.py   # against a running server
```

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `DB_PATH` | `data/daywerk.db` | SQLite file; created on first start |
| `PORT` | `9095` | HTTP port |
| `BIND` | `0.0.0.0` | Bind address — use `127.0.0.1` behind a reverse proxy |
| `WEB_ROOT` | `apps/web-ui/dist` | Static UI directory |
| `ADMIN_EMAIL` | `admin@daywerk.local` | First administrator (only on an empty database) |
| `ADMIN_PASSWORD` | `daywerk` | Password for that administrator |

> Change the administrator password immediately after the first start.

## Roles

| Role | Sees | Can do |
|---|---|---|
| `employee` | own days, own balance | punch, read own statistics, change own profile |
| `office` | **own organisation** | everything above, plus all workers' days and **balances**, corrections, adjustments |
| `admin` | **all organisations** | everything above, plus users, accounts, branding, backup/import |

## Hours balance

For a period (default: the current month):

```
balance = Σ work segments − (target_minutes × days with entries) + adjustments
```

- `target_minutes` comes from the organisation (`daily_target_minutes`, default 480)
  or the worker's own override.
- Only days that actually have a punched entry count towards the target, so a day
  off does not silently create a negative balance.
- `balance_adjustments` lets the office carry over a previous balance or correct one.

## API

All endpoints return JSON and take `Authorization: Bearer <token>` except login.

| Method | Path | Role | Purpose |
|---|---|---|---|
| `POST` | `/api/auth/login` | – | obtain a token |
| `POST` | `/api/auth/logout` | any | invalidate it |
| `GET` | `/api/me` | any | user, org, accounts, balances |
| `GET` | `/api/today` | any | open day, segments, active account |
| `POST` | `/api/punch` | any | `{"action":"start\|pause\|stop","account_id":"…","note":"…","material":"…"}` |
| `GET` | `/api/days?from&to&user_id` | any / office | day history |
| `GET` | `/api/stats?from&to&user_id` | any / office | aggregates + balance |
| `GET` | `/api/team?from&to&org_id` | office / admin | balances of every worker |
| `GET` `POST` | `/api/users` | office / admin | list / create employees |
| `PATCH` | `/api/users/{id}` | admin | role, target, locale, password |
| `PUT` | `/api/profile` | any | own name, locale, password |
| `GET` `POST` | `/api/accounts` | any | list; create a work account in your own org |
| `PATCH` | `/api/accounts/{id}` | admin | rename, archive, restore, reorder |
| `DELETE` | `/api/accounts/{id}` | admin | delete an **unused** account (409 when it has entries) |
| `PATCH` | `/api/segments/{id}` | own today / office / admin | edit note and material |
| `GET` | `/api/bookings?q=&from&to&user_id` | office / admin | search notes and materials |
| `GET` | `/api/export/timesheet.xlsx` | office / admin | Excel report |
| `GET` | `/api/export/timesheet.pdf` | office / admin | PDF report |
| `GET` `PUT` | `/api/org` | any / admin | own organisation |
| `GET` `POST` | `/api/orgs` | admin | all organisations / create |
| `PATCH` | `/api/orgs/{id}` | admin | branding of any organisation |
| `POST` | `/api/adjustments` | office | manual balance correction |
| `GET` `POST` | `/api/export` `/api/import` | admin | JSON backup / restore |

## Data model

```
organizations ─┬─ users ─┬─ workdays ─┬─ segments ── accounts
               │         │            │
               ├─ accounts            └─ (one open segment per user, DB-enforced)
               └─ org_settings
```

Timestamps are UTC milliseconds; the day is derived from the organisation's
timezone. A shift that crosses midnight stays on the day it started. Each
segment carries the `note` and `material` that were entered when it was closed.

## Deployment

```sh
cp deploy/.env.example .env      # set ADMIN_EMAIL and ADMIN_PASSWORD
docker compose up -d --build
```

The service binds to `127.0.0.1:9095`. Put a reverse proxy in front for TLS
(Caddy, nginx) and point it at that port.

## Status

Implemented: domain model and state machine, SQLite storage, REST API, web UI
(punch, history, search, statistics, team, settings, admin), multi-tenancy,
roles, balances, notes and materials, German/English, JSON backup, XLSX and PDF
export, account delete/archive/restore.

Next: Tauri clients with an offline outbox, SMTP for password reset, and editing
the *times* of past segments (notes are already editable).

## License

AGPL-3.0-only. See [LICENSE](LICENSE).
