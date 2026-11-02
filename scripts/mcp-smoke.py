#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# This file is part of Daywerk. See LICENSE for details.

"""End-to-end smoke test for the MCP server (dw-mcp).

Runs against a fresh Daywerk server and drives the MCP protocol over stdio:

    BASE=http://127.0.0.1:9098 python3 scripts/mcp-smoke.py

The server must be empty; the test creates its own data.
"""

import json
import os
import subprocess
import sys
import urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASE = os.environ.get("BASE", "http://127.0.0.1:9098")
ADMIN_EMAIL = os.environ.get("ADMIN_EMAIL", "admin@test.local")
ADMIN_PASSWORD = os.environ.get("ADMIN_PASSWORD", "testtest1234")
BINARY = os.environ.get("DW_MCP", os.path.join(REPO, "target/debug/dw-mcp"))

results: list[str] = []


def check(condition, label):
    results.append(f"{'PASS' if condition else 'FAIL'}  {label}")


def http(path, method="GET", body=None, token=None):
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(f"{BASE}{path}", data=data, method=method)
    if data:
        request.add_header("Content-Type", "application/json")
    if token:
        request.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(request) as response:
        raw = response.read()
        return json.loads(raw) if raw else None


class Mcp:
    def __init__(self, token):
        self.proc = subprocess.Popen(
            [BINARY, "--server", BASE, "--token", token],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        self.next_id = 0

    def rpc(self, method, params=None, notify=False):
        self.next_id += 1
        message = {"jsonrpc": "2.0", "method": method}
        if not notify:
            message["id"] = self.next_id
        if params is not None:
            message["params"] = params
        self.proc.stdin.write(json.dumps(message) + "\n")
        self.proc.stdin.flush()
        if notify:
            return None
        line = self.proc.stdout.readline()
        return json.loads(line)

    def call(self, name, arguments=None):
        response = self.rpc("tools/call", {"name": name, "arguments": arguments or {}})
        result = response.get("result", {})
        text = "".join(part.get("text", "") for part in result.get("content", []))
        # `ok` means success: the protocol flag is isError.
        return not result.get("isError", False), text

    def close(self):
        self.proc.stdin.close()
        self.proc.wait(timeout=5)


def main() -> int:
    admin = http("/api/auth/login", "POST", {"email": ADMIN_EMAIL, "password": ADMIN_PASSWORD})
    token = admin["token"]

    # The assertions below create their own data; leftovers break them.
    if http("/api/days", token=token) or http("/api/projects", token=token):
        print("error: the server already has data; run against a fresh database", file=sys.stderr)
        return 2

    mcp = Mcp(token)
    try:
        init = mcp.rpc("initialize", {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "smoke"}})
        server = init.get("result", {}).get("serverInfo", {})
        check(server.get("name") == "daywerk-mcp", f"initialize ({server.get('name')})")
        mcp.rpc("notifications/initialized", notify=True)

        listing = mcp.rpc("tools/list")
        tools = listing.get("result", {}).get("tools", [])
        names = {tool["name"] for tool in tools}
        check(len(tools) >= 15, f"tools/list ({len(tools)} tools)")
        check(
            {"whoami", "punch", "delete_booking", "create_task", "update_task"} <= names,
            "expected tools are advertised",
        )
        # Every tool needs a schema an agent can use.
        check(all(t.get("inputSchema", {}).get("type") == "object" for t in tools), "all tools have object schemas")

        ok, text = mcp.call("whoami")
        me = json.loads(text)
        check(ok and me["user"]["email"] == ADMIN_EMAIL, "whoami returns the token's user")

        ok, text = mcp.call("list_accounts")
        accounts = json.loads(text)
        work = next(a for a in accounts if a["kind"] == "work")
        check(ok and work["id"], "list_accounts returns the work account")

        ok, text = mcp.call("punch", {"action": "start", "account_id": work["id"]})
        check(ok and json.loads(text)["active"]["account_id"] == work["id"], "punch start")
        ok, text = mcp.call("punch", {"action": "stop"})
        check(ok, "punch stop")

        ok, text = mcp.call("today")
        today = json.loads(text)
        check(ok and today["day"] is not None, "today shows the day")

        ok, text = mcp.call("list_days")
        days = json.loads(text)
        segments = [s for entry in days for s in entry["segments"]]
        check(ok and len(segments) == 1, "list_days returns the booking")

        ok, text = mcp.call("stats")
        stats = json.loads(text)
        check(ok and "worked_ms" in stats.get("period", {}), "stats")

        ok, text = mcp.call("create_project", {"name": "MCP-Test", "description": "from the agent"})
        project = json.loads(text)
        check(ok and project["name"] == "MCP-Test", "create_project")

        ok, text = mcp.call(
            "create_task",
            {"project_id": project["id"], "title": "Vom Agenten", "estimate_minutes": 30},
        )
        task = json.loads(text)
        check(ok and task["title"] == "Vom Agenten", "create_task")

        ok, text = mcp.call("update_task", {"id": task["id"], "status": "in_progress"})
        check(ok and json.loads(text)["status"] == "in_progress", "update_task keeps untouched fields")

        ok, text = mcp.call("add_task_comment", {"task_id": task["id"], "body": "Hallo"})
        check(ok and json.loads(text)["body"] == "Hallo", "add_task_comment")

        ok, text = mcp.call(
            "add_task_effort", {"task_id": task["id"], "minutes": 15, "date": "2026-11-02"}
        )
        check(ok and json.loads(text)["minutes"] == 15, "add_task_effort")

        ok, text = mcp.call("get_task", {"id": task["id"]})
        detail = json.loads(text)
        check(ok and detail["task"]["status"] == "in_progress", "get_task")

        ok, text = mcp.call("search_bookings", {"q": ""})
        check(ok, "search_bookings is allowed for the admin token")

        # Errors come back as tool results, not protocol failures.
        ok, text = mcp.call("get_task", {"id": "does-not-exist"})
        check(not ok and "404" in text, f"unknown task is a tool error ({text[:40]})")

        ok, text = mcp.call("punch", {"action": "start"})
        check(not ok and "account" in text.lower(), "punch without account fails cleanly")

        ok, text = mcp.call("nope")
        check(not ok and "unknown tool" in text, "unknown tool fails cleanly")

        # Delete the booking through MCP and confirm it is gone.
        segment_id = segments[0]["id"]
        ok, text = mcp.call("delete_booking", {"segment_id": segment_id})
        check(ok, "delete_booking")
        ok, text = mcp.call("list_days")
        check(ok and json.loads(text) == [], "the booking is gone")

        unknown = mcp.rpc("tools/does-not-exist")
        check(unknown.get("error", {}).get("code") == -32601, "unknown method is a protocol error")
    finally:
        mcp.close()

    print("\n".join(results))
    return 0 if all(line.startswith("PASS") for line in results) else 1


if __name__ == "__main__":
    sys.exit(main())
