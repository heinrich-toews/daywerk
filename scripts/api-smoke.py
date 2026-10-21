#!/usr/bin/env python3
"""End-to-end smoke test for the Daywerk API.

Run it against a *fresh* database (the assertions assume no other data):

    ADMIN_EMAIL=admin@test.local ADMIN_PASSWORD=test1234 \
      python3 scripts/api-smoke.py

Optional: BASE=http://127.0.0.1:9095
"""

import json
import os
import sys
import urllib.error
import urllib.request

BASE = os.environ.get("BASE", "http://127.0.0.1:9095")
ADMIN_EMAIL = os.environ.get("ADMIN_EMAIL", "admin@daywerk.local")
ADMIN_PASSWORD = os.environ.get("ADMIN_PASSWORD", "daywerk")

results: list[str] = []


def call(path, token=None, method="GET", body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(f"{BASE}{path}", data=data, method=method)
    if data:
        req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req) as res:
            raw = res.read()
            return res.status, (json.loads(raw) if raw else None)
    except urllib.error.HTTPError as exc:
        raw = exc.read()
        try:
            payload = json.loads(raw)
        except Exception:
            payload = raw.decode(errors="replace")
        return exc.code, payload


def check(condition, label):
    results.append(f"{'PASS' if condition else 'FAIL'}  {label}")


def main() -> int:
    status, admin = call(
        "/api/auth/login", method="POST", body={"email": ADMIN_EMAIL, "password": ADMIN_PASSWORD}
    )
    check(status == 200, f"admin login ({status})")
    if status != 200:
        print("\n".join(results))
        return 1
    admin_token = admin["token"]
    org_a = admin["org"]["id"]

    status, org_b = call(
        "/api/orgs", admin_token, "POST", {"name": "Zweite Firma", "slug": "zweite"}
    )
    check(status == 200, f"admin creates a second organisation ({status})")

    status, _ = call("/api/orgs", admin_token, "POST", {"name": "Doppelt", "slug": "zweite"})
    check(status == 409, f"duplicate slug rejected ({status})")

    for email, name, role, org in [
        ("buero@test.local", "Büro A", "buero", org_a),
        ("worker@test.local", "Worker A", "allgemein", org_a),
        ("other@test.local", "Worker B", "allgemein", org_b["id"]),
    ]:
        status, _ = call(
            "/api/users",
            admin_token,
            "POST",
            {"email": email, "name": name, "password": "test1234", "role": role, "org_id": org},
        )
        check(status == 200, f"create {role} {email} ({status})")

    tokens = {}
    for email in ["buero@test.local", "worker@test.local", "other@test.local"]:
        status, res = call(
            "/api/auth/login", method="POST", body={"email": email, "password": "test1234"}
        )
        check(status == 200, f"login {email} ({status})")
        tokens[email] = res["token"]

    status, _ = call("/api/team", tokens["worker@test.local"])
    check(status == 403, f"employee cannot read the team view ({status})")

    status, team = call("/api/team", tokens["buero@test.local"])
    names = sorted(row["user"]["name"] for row in team)
    check(
        status == 200 and names == ["Administrator", "Büro A", "Worker A"],
        f"office sees exactly its own organisation: {names} ({status})",
    )

    status, team_b = call(f"/api/team?org_id={org_b['id']}", tokens["buero@test.local"])
    check(all(row["org_id"] == org_a for row in team_b), "office cannot escape its organisation")

    status, team = call("/api/team", admin_token)
    check(status == 200 and len(team) == 4, f"admin sees all workers ({len(team)})")

    status, team_b = call(f"/api/team?org_id={org_b['id']}", admin_token)
    check(
        status == 200 and len(team_b) == 1 and team_b[0]["user"]["email"] == "other@test.local",
        f"admin filters by organisation ({len(team_b)})",
    )

    accounts = call("/api/me", tokens["worker@test.local"])[1]["accounts"]
    work = next(a for a in accounts if a["kind"] == "work")

    status, day = call(
        "/api/punch", tokens["worker@test.local"], "POST",
        {"action": "start", "account_id": work["id"]},
    )
    check(status == 200 and day["active"]["account_name"] == "ALLGEMEIN", f"start work ({status})")

    status, day = call("/api/punch", tokens["worker@test.local"], "POST", {"action": "pause"})
    check(status == 200 and day["active"]["kind"] == "break", f"pause ({status})")

    status, day = call(
        "/api/punch", tokens["worker@test.local"], "POST",
        {"action": "start", "account_id": work["id"]},
    )
    check(status == 200 and day["active"]["kind"] == "work", f"resume from pause ({status})")

    status, day = call("/api/punch", tokens["worker@test.local"], "POST", {"action": "stop"})
    check(
        status == 200 and day["day"]["day"]["status"] == "closed",
        f"stop closes the day ({status})",
    )

    status, team = call("/api/team", tokens["buero@test.local"])
    worker = next(row for row in team if row["user"]["email"] == "worker@test.local")
    check(
        worker["period"]["days"] == 1 and worker["period"]["balance_ms"] < 0,
        f"office sees the worker balance ({round(worker['period']['balance_ms'] / 60000)} min)",
    )

    status, _ = call(f"/api/days?user_id={team[0]['user']['id']}", tokens["worker@test.local"])
    check(status == 403, f"employee cannot read another user id ({status})")

    status, me = call("/api/profile", tokens["worker@test.local"], "PUT", {"locale": "en"})
    check(status == 200 and me["locale"] == "en", f"locale switch persists ({status})")

    status, _ = call("/api/export", tokens["worker@test.local"])
    check(status == 403, f"employee cannot export ({status})")

    status, backup = call("/api/export", admin_token)
    check(
        status == 200 and backup["format"] == "daywerk-backup",
        f"admin exports a backup ({status})",
    )
    check(
        len(backup["users"]) == 4 and len(backup["organizations"]) == 2,
        "backup contains 4 users and 2 organisations",
    )

    status, _ = call("/api/import", admin_token, "POST", {"format": "nope"})
    check(status == 400, f"invalid import rejected ({status})")

    status, counts = call("/api/import", admin_token, "POST", backup)
    check(status == 200 and counts["users"] == 4, f"import restores 4 users ({status})")

    print("\n".join(results))
    return 0 if all(line.startswith("PASS") for line in results) else 1


if __name__ == "__main__":
    sys.exit(main())
