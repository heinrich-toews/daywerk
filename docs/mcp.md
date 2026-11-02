# Daywerk MCP server

`dw-mcp` exposes Daywerk as a [Model Context Protocol](https://modelcontextprotocol.io/)
server so AI assistants can read and write time bookings and tasks.

It is a **thin client of the Daywerk HTTP API**, not a database tool. Unlike
`sb-mcp` in OpenChordBook (which opens the SQLite file), Daywerk is
multi-tenant: the MCP server acts as the user whose token it was given, and all
permission checks, tenant isolation, time zones and auditing stay on the server.
Every tool is exactly one API call.

## Build and run

```sh
cargo build --release -p dw-mcp
./target/release/dw-mcp --server https://time.hgs-t.de --token <token>
```

Configuration, in order of precedence: command-line flags, environment,
built-in default.

| Setting | Flag | Environment | Default |
| --- | --- | --- | --- |
| Server | `--server` | `DAYWERK_SERVER` | `https://time.hgs-t.de` |
| Token | `--token` | `DAYWERK_TOKEN` | – |
| E-mail / password | `--email`, `--password` | `DAYWERK_EMAIL`, `DAYWERK_PASSWORD` | – |

A token is preferred. Without one, the server logs in with the credentials and
keeps the session token in memory. `dw-mcp --login --email … --password …`
prints a fresh token and exits, so a config can be filled without `curl`.
Tokens stay valid until that session is logged out, so they suit a config file.

Startup validates the token against `/api/me` and exits with a clear message if
it is invalid. Transport is JSON-RPC 2.0 over **stdio**; diagnostics go to
stderr only.

## Registering with opencode

```jsonc
{
  "mcp": {
    "servers": {
      "daywerk": {
        "type": "local",
        "command": ["/path/to/dw-mcp"],
        "environment": {
          "DAYWERK_SERVER": "https://time.hgs-t.de",
          "DAYWERK_TOKEN": "<token>"
        }
      }
    }
  }
}
```

Tools are then available as `daywerk_<tool>` (or grouped under
`tools.daywerk` in Code Mode). Claude Desktop uses the same command/environment
pair in its own `mcpServers` block.

## Tools

Read-only:

| Tool | Arguments | Returns |
| --- | --- | --- |
| `whoami` | – | user, organisation, role, accounts, balances |
| `today` | – | today's day, running segment, totals and balances |
| `list_accounts` | – | active time accounts (use the id for `punch`) |
| `list_days` | `from?`, `to?`, `user_id?` | days with segments and totals |
| `search_bookings` | `q?`, `from?`, `to?`, `user_id?`, `account_id?`, `limit?` | bookings (office/admin) |
| `stats` | `from?`, `to?`, `user_id?` | period/total worked, target, balance |
| `team` | – | balances of all workers (office/admin) |
| `list_projects` | – | projects with linked account and counts |
| `list_tasks` | `project_id?`, `status?`, `assignee_id?`, `q?`, `limit?` | task rows |
| `get_task` | `id` | task with subtasks, comments and effort |

Mutating:

| Tool | Arguments | Notes |
| --- | --- | --- |
| `punch` | `action` (start/pause/stop), `account_id?`, `note?`, `material?` | `account_id` required for start |
| `delete_booking` | `segment_id` | own bookings; office/admin per role |
| `create_task` | `project_id`, `title`, plus optional fields | subtasks via `parent_id` |
| `update_task` | `id` plus any fields | reads the task first, PATCH is a full update |
| `add_task_comment` | `task_id`, `body` | |
| `add_task_effort` | `task_id`, `minutes`, `date`, `note?` | manual effort, not a timer |
| `create_project` | `name`, `description?`, `account_id?` | office/admin |
| `adjust_balance` | `user_id`, `date`, `minutes`, `reason?` | office/admin |

Deliberately not exposed: purge, user/account/org administration, import,
exports. An agent cannot escalate beyond the token's role.

## Contracts and behaviour

- **Errors** are tool results with `isError: true` and the server's message
  (for example `HTTP 403: forbidden`), so the agent can react. JSON-RPC errors
  are reserved for protocol problems.
- **Validation:** missing or wrong-typed arguments produce a clear `isError`
  result; the server never panics.
- **Time zones and dates** are the server's (organisation time zone). `today`
  and `stats` default to the current day/month.
- **Audit:** the server records the same audit entries as the UI; the MCP adds
  no side channel.
- **Network:** the server must be reachable; there is no offline cache.

## Tests

```sh
# fresh database on :9098
BASE=http://127.0.0.1:9098 python3 scripts/mcp-smoke.py
```

The script refuses to run against a database that already has data, drives
initialize → tools/list → 20 tool calls over stdio, and covers success and error
paths.
