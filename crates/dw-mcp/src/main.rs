// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

//! Daywerk MCP server — Model Context Protocol over stdio.
//!
//! Unlike `sb-mcp` in OpenChordBook (which opens the SQLite file), Daywerk is
//! multi-tenant: this binary is a *client* of the Daywerk HTTP API and acts as
//! the user whose token it was given. Permissions, tenant isolation, time zones
//! and auditing stay in the server; every tool is one API call.
//!
//! Usage: `dw-mcp --server https://time.hgs-t.de --token <token>`
//! Transport: JSON-RPC 2.0 over stdin/stdout (MCP stdio transport).

use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value, from_str, json};

const DEFAULT_SERVER: &str = "https://time.hgs-t.de";
const PROTOCOL: &str = "2025-11-25";
const TIMEOUT_SECS: u64 = 20;

// ---- Protocol types ----------------------------------------------------

#[derive(Deserialize)]
struct RpcRequest {
    id: Option<Value>,
    method: Option<String>,
    params: Option<Value>,
}

#[derive(Deserialize, Default)]
struct ToolCallParams {
    name: Option<String>,
    arguments: Option<Map<String, Value>>,
}

// ---- HTTP client -------------------------------------------------------

struct Api {
    base: String,
    token: String,
    agent: ureq::Agent,
}

impl Api {
    fn new(server: &str, token: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build();
        Self {
            base: server.trim_end_matches('/').to_string(),
            token: token.to_string(),
            agent,
        }
    }

    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let req = self
            .agent
            .request(method, &format!("{}{}", self.base, path))
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Accept", "application/json");
        let result = match body {
            Some(value) => req
                .set("Content-Type", "application/json")
                .send_json(value.clone()),
            None => req.call(),
        };

        match result {
            Ok(response) => {
                let text = response
                    .into_string()
                    .map_err(|e| format!("cannot read response: {e}"))?;
                if text.trim().is_empty() {
                    Ok(Value::Null)
                } else {
                    from_str(&text).map_err(|e| format!("invalid JSON from server: {e}"))
                }
            }
            Err(ureq::Error::Status(code, response)) => {
                let text = response.into_string().unwrap_or_default();
                let message = from_str::<Value>(&text)
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                    .unwrap_or_else(|| text.trim().to_string());
                Err(format!("HTTP {code}: {message}"))
            }
            Err(err) => Err(format!("request failed: {err}")),
        }
    }

    fn get(&self, path: &str) -> Result<Value, String> {
        self.request("GET", path, None)
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.request("POST", path, Some(body))
    }

    fn patch(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.request("PATCH", path, Some(body))
    }

    fn delete(&self, path: &str) -> Result<Value, String> {
        self.request("DELETE", path, None)
    }
}

/// Log in once and hand back a session token (used when no token is configured).
fn login(server: &str, email: &str, password: &str) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build();
    let response = agent
        .post(&format!("{}/api/auth/login", server.trim_end_matches('/')))
        .send_json(json!({ "email": email, "password": password }))
        .map_err(|err| match err {
            ureq::Error::Status(401, _) => "login failed: wrong e-mail or password".to_string(),
            ureq::Error::Status(code, _) => format!("login failed: HTTP {code}"),
            other => format!("login failed: {other}"),
        })?;
    let value: Value = response
        .into_json()
        .map_err(|e| format!("login returned invalid JSON: {e}"))?;
    value
        .get("token")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .ok_or_else(|| "login returned no token".to_string())
}

// ---- Entry point -------------------------------------------------------

fn usage() {
    eprint!(
        "dw-mcp {}\n\
         \n\
         Usage: dw-mcp [--server URL] [--token TOKEN | --email MAIL --password PASS] [--login]\n\
         \n\
         Environment: DAYWERK_SERVER, DAYWERK_TOKEN, DAYWERK_EMAIL, DAYWERK_PASSWORD\n\
         `--login` prints a fresh token and exits.\n",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut server = std::env::var("DAYWERK_SERVER").unwrap_or_else(|_| DEFAULT_SERVER.into());
    let mut token = std::env::var("DAYWERK_TOKEN").unwrap_or_default();
    let mut email = std::env::var("DAYWERK_EMAIL").unwrap_or_default();
    let mut password = std::env::var("DAYWERK_PASSWORD").unwrap_or_default();
    let mut login_only = false;

    let mut index = 0;
    while index < args.len() {
        let value = |offset: usize| -> String {
            args.get(index + offset).cloned().unwrap_or_default()
        };
        match args[index].as_str() {
            "--server" => {
                server = value(1);
                index += 2;
            }
            "--token" => {
                token = value(1);
                index += 2;
            }
            "--email" => {
                email = value(1);
                index += 2;
            }
            "--password" => {
                password = value(1);
                index += 2;
            }
            "--login" => {
                login_only = true;
                index += 1;
            }
            "--help" | "-h" => {
                usage();
                return;
            }
            other => {
                eprintln!("error: unknown argument: {other}");
                usage();
                std::process::exit(2);
            }
        }
    }

    if login_only {
        if email.is_empty() || password.is_empty() {
            eprintln!("error: --login needs --email and --password (or the env vars)");
            std::process::exit(2);
        }
        match login(&server, &email, &password) {
            Ok(token) => println!("{token}"),
            Err(err) => {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
        return;
    }

    if token.is_empty() {
        if email.is_empty() || password.is_empty() {
            eprintln!(
                "error: no token. Set DAYWERK_TOKEN, or DAYWERK_EMAIL/DAYWERK_PASSWORD, \
                 or pass --token/--email/--password. `dw-mcp --login` prints a token."
            );
            std::process::exit(2);
        }
        match login(&server, &email, &password) {
            Ok(fresh) => token = fresh,
            Err(err) => {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
    }

    let api = Api::new(&server, &token);
    match api.get("/api/me") {
        Ok(me) => {
            let user = me.get("user").cloned().unwrap_or(Value::Null);
            eprintln!(
                "dw-mcp ready: {} ({}) at {}",
                user.get("email").and_then(|v| v.as_str()).unwrap_or("?"),
                user.get("role").and_then(|v| v.as_str()).unwrap_or("?"),
                server
            );
        }
        Err(err) => {
            eprintln!("error: cannot use the Daywerk API: {err}");
            std::process::exit(1);
        }
    }

    serve(&api);
}

fn serve(api: &Api) {
    let stdin = BufReader::new(std::io::stdin());
    for line in stdin.lines() {
        let Ok(line) = line else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(request) = from_str::<RpcRequest>(trimmed) else {
            respond_err(Value::Null, -32700, "Parse error");
            continue;
        };

        let id = request.id.clone().unwrap_or(Value::Null);
        let method = request.method.clone().unwrap_or_default();

        match method.as_str() {
            "initialize" | "server/discover" => respond_ok(id, init_result()),
            "tools/list" => respond_ok(id, tools_result()),
            "tools/call" => {
                let params: ToolCallParams = request
                    .params
                    .clone()
                    .and_then(|p| from_str(&p.to_string()).ok())
                    .unwrap_or_default();
                let name = params.name.unwrap_or_default();
                let arguments = params.arguments.unwrap_or_default();
                let (ok, payload) = exec(api, &name, &arguments);
                respond_ok(id, call_result(&payload, !ok));
            }
            "ping" => respond_ok(id, json!({})),
            // Notifications have no id and must not be answered.
            _ if request.id.is_none() => {}
            _ => respond_err(id, -32601, &format!("Method not found: {method}")),
        }
    }
}

// ---- JSON-RPC ----------------------------------------------------------

fn respond_ok(id: Value, result: Value) {
    out(&format!(
        "{}\n",
        serde_json::to_string(&json!({ "jsonrpc": "2.0", "id": id, "result": result }))
            .unwrap_or_default()
    ));
}

fn respond_err(id: Value, code: i32, message: &str) {
    out(&format!(
        "{}\n",
        serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }))
        .unwrap_or_default()
    ));
}

fn out(text: &str) {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(text.as_bytes()).ok();
    lock.flush().ok();
}

fn call_result(text: &str, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    })
}

// ---- MCP results -------------------------------------------------------

fn init_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL,
        "serverInfo": { "name": "daywerk-mcp", "version": env!("CARGO_PKG_VERSION") },
        "capabilities": { "tools": {} },
        "instructions": "Daywerk time tracking and tasks. You act as the user whose token was configured: \
                         punching, editing and deleting follow that user's role (employee, office, admin). \
                         Destructive administration (purge, users, import) is not exposed."
    })
}

fn tools_result() -> Value {
    json!({ "tools": tool_defs() })
}

fn tool(name: &str, description: &str, schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": schema })
}

fn tool_defs() -> Vec<Value> {
    vec![
        tool(
            "whoami",
            "Current user, organisation, role, time accounts and balances.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "today",
            "Today's punch clock state: current day, running segment, totals and balances (server time zone).",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "list_accounts",
            "Active time accounts. Use the id when calling punch.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "list_days",
            "Booked days with segments and totals. Defaults to the current month.",
            json!({
                "type": "object",
                "properties": {
                    "from": { "type": "string", "description": "YYYY-MM-DD" },
                    "to": { "type": "string", "description": "YYYY-MM-DD" },
                    "user_id": { "type": "string", "description": "office/admin only" }
                }
            }),
        ),
        tool(
            "search_bookings",
            "Search bookings by note or material. Requires office or admin.",
            json!({
                "type": "object",
                "properties": {
                    "q": { "type": "string" },
                    "from": { "type": "string" },
                    "to": { "type": "string" },
                    "user_id": { "type": "string" },
                    "account_id": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200 }
                }
            }),
        ),
        tool(
            "stats",
            "Worked time, target and balance for a range (defaults to the current month).",
            json!({
                "type": "object",
                "properties": {
                    "from": { "type": "string" },
                    "to": { "type": "string" },
                    "user_id": { "type": "string" }
                }
            }),
        ),
        tool(
            "team",
            "Balances of every worker. Office sees its organisation, admin sees all.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "list_projects",
            "Projects with their linked time account and task counts.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "list_tasks",
            "Tasks, optionally filtered by project, status, assignee or text.",
            json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "status": { "type": "string", "enum": ["open", "in_progress", "done", "cancelled"] },
                    "assignee_id": { "type": "string" },
                    "q": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500 }
                }
            }),
        ),
        tool(
            "get_task",
            "One task with subtasks, comments and recorded effort.",
            json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            }),
        ),
        tool(
            "punch",
            "Start, pause or stop the timer. Closing a work segment can carry note and material.",
            json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["start", "pause", "stop"] },
                    "account_id": { "type": "string", "description": "required for start" },
                    "note": { "type": "string" },
                    "material": { "type": "string" }
                },
                "required": ["action"]
            }),
        ),
        tool(
            "delete_booking",
            "Delete one booking. Employees only their own; office their organisation; admin any.",
            json!({
                "type": "object",
                "properties": { "segment_id": { "type": "string" } },
                "required": ["segment_id"]
            }),
        ),
        tool(
            "create_task",
            "Create a task (or subtask with parent_id) in a project.",
            json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string" },
                    "parent_id": { "type": "string" },
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "assignee_id": { "type": "string" },
                    "start_date": { "type": "string" },
                    "due_date": { "type": "string" },
                    "priority": { "type": "string", "enum": ["low", "normal", "high", "urgent"] },
                    "estimate_minutes": { "type": "integer", "minimum": 0 }
                },
                "required": ["project_id", "title"]
            }),
        ),
        tool(
            "update_task",
            "Change task fields; only the given fields are touched.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "assignee_id": { "type": "string" },
                    "start_date": { "type": "string" },
                    "due_date": { "type": "string" },
                    "priority": { "type": "string", "enum": ["low", "normal", "high", "urgent"] },
                    "status": { "type": "string", "enum": ["open", "in_progress", "done", "cancelled"] },
                    "estimate_minutes": { "type": "integer", "minimum": 0 }
                },
                "required": ["id"]
            }),
        ),
        tool(
            "add_task_comment",
            "Add a comment to a task.",
            json!({
                "type": "object",
                "properties": {
                    "task_id": { "type": "string" },
                    "body": { "type": "string" }
                },
                "required": ["task_id", "body"]
            }),
        ),
        tool(
            "add_task_effort",
            "Record manual effort on a task (minutes, date, optional note).",
            json!({
                "type": "object",
                "properties": {
                    "task_id": { "type": "string" },
                    "minutes": { "type": "integer", "minimum": 1 },
                    "date": { "type": "string", "description": "YYYY-MM-DD" },
                    "note": { "type": "string" }
                },
                "required": ["task_id", "minutes", "date"]
            }),
        ),
        tool(
            "create_project",
            "Create a project. Requires office or admin.",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "account_id": { "type": "string", "description": "linked time account" }
                },
                "required": ["name"]
            }),
        ),
        tool(
            "adjust_balance",
            "Add or remove balance minutes for a user on a date. Requires office or admin.",
            json!({
                "type": "object",
                "properties": {
                    "user_id": { "type": "string" },
                    "date": { "type": "string" },
                    "minutes": { "type": "integer" },
                    "reason": { "type": "string" }
                },
                "required": ["user_id", "date", "minutes"]
            }),
        ),
    ]
}

// ---- Tool execution ----------------------------------------------------

fn exec(api: &Api, name: &str, args: &Map<String, Value>) -> (bool, String) {
    match name {
        "whoami" => finish(api.get("/api/me")),
        "today" => finish(api.get("/api/today")),
        "list_accounts" => finish(api.get("/api/accounts")),
        "list_days" => {
            let query = Query::new()
                .add("from", text(args, "from"))
                .add("to", text(args, "to"))
                .add("user_id", text(args, "user_id"));
            finish(api.get(&format!("/api/days{}", query.finish())))
        }
        "search_bookings" => {
            let query = Query::new()
                .add("q", text(args, "q"))
                .add("from", text(args, "from"))
                .add("to", text(args, "to"))
                .add("user_id", text(args, "user_id"))
                .add("account_id", text(args, "account_id"))
                .add("limit", number(args, "limit").map(|n| n.to_string()).as_deref());
            finish(api.get(&format!("/api/bookings{}", query.finish())))
        }
        "stats" => {
            let query = Query::new()
                .add("from", text(args, "from"))
                .add("to", text(args, "to"))
                .add("user_id", text(args, "user_id"));
            finish(api.get(&format!("/api/stats{}", query.finish())))
        }
        "team" => finish(api.get("/api/team")),
        "list_projects" => finish(api.get("/api/projects")),
        "list_tasks" => {
            let query = Query::new()
                .add("project_id", text(args, "project_id"))
                .add("status", text(args, "status"))
                .add("assignee_id", text(args, "assignee_id"))
                .add("q", text(args, "q"))
                .add("limit", number(args, "limit").map(|n| n.to_string()).as_deref());
            finish(api.get(&format!("/api/tasks{}", query.finish())))
        }
        "get_task" => with_id(api, args, "id", |id| format!("/api/tasks/{id}"), Api::get),
        "punch" => {
            let action = match required(args, "action") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let mut body = json!({ "action": action });
            if let Some(value) = text(args, "account_id") {
                body["account_id"] = json!(value);
            }
            if let Some(value) = text(args, "note") {
                body["note"] = json!(value);
            }
            if let Some(value) = text(args, "material") {
                body["material"] = json!(value);
            }
            finish(api.post("/api/punch", &body))
        }
        "delete_booking" => match required(args, "segment_id") {
            Ok(id) => finish(api.delete(&format!("/api/segments/{}", encode(id)))),
            Err(err) => (false, err),
        },
        "create_task" => {
            let project_id = match required(args, "project_id") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let title = match required(args, "title") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let mut body = json!({ "project_id": project_id, "title": title });
            for key in ["parent_id", "description", "assignee_id", "start_date", "due_date"] {
                if let Some(value) = text(args, key) {
                    body[key] = json!(value);
                }
            }
            if let Some(value) = text(args, "priority") {
                body["priority"] = json!(value);
            }
            if let Some(value) = number(args, "estimate_minutes") {
                body["estimate_minutes"] = json!(value);
            }
            finish(api.post("/api/tasks", &body))
        }
        "update_task" => update_task(api, args),
        "add_task_comment" => {
            let task_id = match required(args, "task_id") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let body = match required(args, "body") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            finish(api.post(&format!("/api/tasks/{}/comments", encode(task_id)), &json!({ "body": body })))
        }
        "add_task_effort" => {
            let task_id = match required(args, "task_id") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let minutes = match number(args, "minutes") {
                Some(value) => value,
                None => return (false, "missing required argument: minutes".into()),
            };
            let date = match required(args, "date") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let body = json!({
                "minutes": minutes,
                "spent_on": date,
                "note": text(args, "note").unwrap_or_default(),
            });
            finish(api.post(&format!("/api/tasks/{}/time", encode(task_id)), &body))
        }
        "create_project" => {
            let name = match required(args, "name") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let mut body = json!({ "name": name });
            if let Some(value) = text(args, "description") {
                body["description"] = json!(value);
            }
            if let Some(value) = text(args, "account_id") {
                body["account_id"] = json!(value);
            }
            finish(api.post("/api/projects", &body))
        }
        "adjust_balance" => {
            let user_id = match required(args, "user_id") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let date = match required(args, "date") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let minutes = match number(args, "minutes") {
                Some(value) => value,
                None => return (false, "missing required argument: minutes".into()),
            };
            let body = json!({
                "user_id": user_id,
                "date": date,
                "minutes": minutes,
                "reason": text(args, "reason").unwrap_or_default(),
            });
            finish(api.post("/api/adjustments", &body))
        }
        other => (false, format!("unknown tool: {other}")),
    }
}

/// The task PATCH replaces the whole record, so read it and merge.
fn update_task(api: &Api, args: &Map<String, Value>) -> (bool, String) {
    let id = match required(args, "id") {
        Ok(value) => value,
        Err(err) => return (false, err),
    };
    let current = match api.get(&format!("/api/tasks/{}", encode(id))) {
        Ok(value) => value,
        Err(err) => return (false, err),
    };
    let task = current.get("task").cloned().unwrap_or(current);

    let mut body = json!({
        "title": task.get("title").cloned().unwrap_or(json!("")),
        "description": task.get("description").cloned().unwrap_or(json!("")),
        "assignee_id": task.get("assignee_id").cloned().unwrap_or(Value::Null),
        "start_date": task.get("start_date").cloned().unwrap_or(Value::Null),
        "due_date": task.get("due_date").cloned().unwrap_or(Value::Null),
        "priority": task.get("priority").cloned().unwrap_or(json!("normal")),
        "status": task.get("status").cloned().unwrap_or(json!("open")),
        "estimate_minutes": task.get("estimate_minutes").cloned().unwrap_or(Value::Null),
    });

    for key in ["title", "description", "assignee_id", "start_date", "due_date"] {
        if let Some(value) = text(args, key) {
            body[key] = json!(value);
        }
    }
    if let Some(value) = text(args, "priority") {
        body["priority"] = json!(value);
    }
    if let Some(value) = text(args, "status") {
        body["status"] = json!(value);
    }
    if let Some(value) = number(args, "estimate_minutes") {
        body["estimate_minutes"] = json!(value);
    }

    finish(api.patch(&format!("/api/tasks/{}", encode(id)), &body))
}

fn with_id<F>(api: &Api, args: &Map<String, Value>, key: &str, path: impl Fn(&str) -> String, call: F) -> (bool, String)
where
    F: Fn(&Api, &str) -> Result<Value, String>,
{
    match required(args, key) {
        Ok(id) => finish(call(api, &path(id))),
        Err(err) => (false, err),
    }
}

fn finish(result: Result<Value, String>) -> (bool, String) {
    match result {
        Ok(value) => (
            true,
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| "null".into()),
        ),
        Err(err) => (false, err),
    }
}

// ---- helpers -----------------------------------------------------------

fn text<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|value| value.as_str())
}

fn number(args: &Map<String, Value>, key: &str) -> Option<i64> {
    args.get(key).and_then(|value| value.as_i64())
}

fn required<'a>(args: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    text(args, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing required argument: {key}"))
}

struct Query {
    parts: Vec<String>,
}

impl Query {
    fn new() -> Self {
        Self { parts: Vec::new() }
    }

    fn add(mut self, key: &str, value: Option<&str>) -> Self {
        if let Some(value) = value
            && !value.is_empty()
        {
            self.parts.push(format!("{key}={}", encode(value)));
        }
        self
    }

    fn finish(self) -> String {
        if self.parts.is_empty() {
            String::new()
        } else {
            format!("?{}", self.parts.join("&"))
        }
    }
}

/// Minimal percent-encoding for query values and path segments.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
