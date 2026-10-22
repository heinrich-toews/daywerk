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
from urllib.parse import quote

BASE = os.environ.get("BASE", "http://127.0.0.1:9095")
ADMIN_EMAIL = os.environ.get("ADMIN_EMAIL", "admin@daywerk.local")
ADMIN_PASSWORD = os.environ.get("ADMIN_PASSWORD", "daywerk")

results: list[str] = []


def call(path, token=None, method="GET", body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(f"{BASE}{quote(path, safe='/?&=')}", data=data, method=method)
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


def fetch_raw(path, token):
    req = urllib.request.Request(f"{BASE}{quote(path, safe='/?&=')}")
    req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req) as res:
            return res.status, res.headers.get("content-type", ""), res.read()
    except urllib.error.HTTPError as exc:
        return exc.code, exc.headers.get("content-type", ""), exc.read()


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
        "/api/orgs", admin_token, "POST", {"name": "Second Company", "slug": "second"}
    )
    check(status == 200, f"admin creates a second organisation ({status})")

    status, _ = call("/api/orgs", admin_token, "POST", {"name": "Duplicate", "slug": "second"})
    check(status == 409, f"duplicate slug rejected ({status})")

    for email, name, role, org in [
        ("office@test.local", "Office A", "office", org_a),
        ("worker@test.local", "Worker A", "employee", org_a),
        ("other@test.local", "Worker B", "employee", org_b["id"]),
    ]:
        status, _ = call(
            "/api/users",
            admin_token,
            "POST",
            {"email": email, "name": name, "password": "test1234", "role": role, "org_id": org},
        )
        check(status == 200, f"create {role} {email} ({status})")

    tokens = {}
    for email in ["office@test.local", "worker@test.local", "other@test.local"]:
        status, res = call(
            "/api/auth/login", method="POST", body={"email": email, "password": "test1234"}
        )
        check(status == 200, f"login {email} ({status})")
        tokens[email] = res["token"]

    status, _ = call("/api/team", tokens["worker@test.local"])
    check(status == 403, f"employee cannot read the team view ({status})")

    status, team = call("/api/team", tokens["office@test.local"])
    names = sorted(row["user"]["name"] for row in team)
    check(
        status == 200 and names == ["Administrator", "Office A", "Worker A"],
        f"office sees exactly its own organisation: {names} ({status})",
    )

    status, team_b = call(f"/api/team?org_id={org_b['id']}", tokens["office@test.local"])
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

    status, team = call("/api/team", tokens["office@test.local"])
    worker = next(row for row in team if row["user"]["email"] == "worker@test.local")
    check(
        worker["period"]["days"] == 1 and worker["period"]["balance_ms"] < 0,
        f"office sees the worker balance ({round(worker['period']['balance_ms'] / 60000)} min)",
    )

    status, _ = call(f"/api/days?user_id={team[0]['user']['id']}", tokens["worker@test.local"])
    check(status == 403, f"employee cannot read another user id ({status})")

    # ---- employees may create work accounts themselves -----------------
    worker_token = tokens["worker@test.local"]
    status, own_account = call(
        "/api/accounts", worker_token, "POST", {"name": "Lager", "kind": "work"}
    )
    check(status == 200, f"employee creates a work account ({status})")

    status, _ = call("/api/accounts", worker_token, "POST", {"name": "lager", "kind": "work"})
    check(status == 409, f"duplicate active account name rejected ({status})")

    status, _ = call("/api/accounts", worker_token, "POST", {"name": "Extra", "kind": "break"})
    check(status == 403, f"employee cannot create a break account ({status})")

    # ---- notes and materials on a transition ---------------------------
    status, day = call(
        "/api/punch", worker_token, "POST",
        {"action": "start", "account_id": work["id"]},
    )
    check(status == 200, f"reopen the closed day ({status})")

    status, day = call(
        "/api/punch", worker_token, "POST",
        {
            "action": "start",
            "account_id": own_account["id"],
            "note": "Regal aufgefüllt",
            "material": "2x Kabel 3m",
        },
    )
    check(status == 200, f"switch with note and material ({status})")

    status, days = call("/api/days", worker_token)
    annotated = [
        seg for entry in days for seg in entry["segments"] if seg["note"] == "Regal aufgefüllt"
    ]
    check(
        len(annotated) == 1 and annotated[0]["material"] == "2x Kabel 3m",
        "note and material stored on the closed segment",
    )

    status, updated = call(
        f"/api/segments/{annotated[0]['id']}", worker_token, "PATCH",
        {"note": "Regal aufgefüllt (korrigiert)"},
    )
    check(
        status == 200 and updated["note"] == "Regal aufgefüllt (korrigiert)"
        and updated["material"] == "2x Kabel 3m",
        f"employee edits the note of today's segment ({status})",
    )

    # ---- search --------------------------------------------------------
    status, _ = call("/api/bookings?q=kabel", worker_token)
    check(status == 403, f"employee cannot search bookings ({status})")

    status, found = call("/api/bookings?q=kabel", tokens["office@test.local"])
    check(status == 200 and len(found) == 1, f"office finds the booking ({status}, {len(found)})")

    status, found = call("/api/bookings?q=KABEL", tokens["office@test.local"])
    check(status == 200 and len(found) == 1, "search is case-insensitive")

    status, found = call("/api/bookings?q=aufgefüllt", tokens["office@test.local"])
    check(status == 200 and len(found) == 1, "search folds umlauts")

    status, found = call(
        f"/api/bookings?q=kabel&org_id={org_b['id']}", tokens["office@test.local"]
    )
    check(len(found) == 1, "office stays inside its own organisation while searching")

    status, found = call(f"/api/bookings?q=kabel&org_id={org_b['id']}", admin_token)
    check(len(found) == 0, "admin can filter by another organisation")

    status, found = call("/api/bookings?q=kabel", admin_token)
    check(len(found) == 1, f"admin searches across organisations ({len(found)})")

    status, me = call("/api/profile", tokens["worker@test.local"], "PUT", {"locale": "en"})
    check(status == 200 and me["locale"] == "en", f"locale switch persists ({status})")

    status, _ = call("/api/export", tokens["worker@test.local"])
    check(status == 403, f"employee cannot export ({status})")

    # ---- account deletion ----------------------------------------------
    status, temp = call(
        "/api/accounts", worker_token, "POST", {"name": "Tippfehler", "kind": "work"}
    )
    check(status == 200, f"create a throwaway account ({status})")
    status, _ = call(f"/api/accounts/{temp['id']}", worker_token, "DELETE")
    check(status == 403, f"employee cannot delete accounts ({status})")
    status, _ = call(f"/api/accounts/{temp['id']}", admin_token, "DELETE")
    check(status == 204, f"admin deletes an unused account ({status})")
    status, _ = call(f"/api/accounts/{own_account['id']}", admin_token, "DELETE")
    check(status == 409, f"used account is protected ({status})")

    # ---- timesheet exports ---------------------------------------------
    status, ctype, body = fetch_raw("/api/export/timesheet.xlsx", worker_token)
    check(status == 403, f"employee cannot export a timesheet ({status})")

    status, ctype, body = fetch_raw("/api/export/timesheet.xlsx", tokens["office@test.local"])
    check(
        status == 200 and body[:2] == b"PK" and "spreadsheet" in ctype,
        f"office downloads xlsx ({status}, {len(body)} bytes)",
    )

    status, ctype, body = fetch_raw("/api/export/timesheet.pdf", tokens["office@test.local"])
    check(
        status == 200 and body.startswith(b"%PDF-") and "pdf" in ctype,
        f"office downloads pdf ({status}, {len(body)} bytes)",
    )

    status, ctype, body = fetch_raw("/api/export/timesheet.xlsx", admin_token)
    check(status == 200 and body[:2] == b"PK", f"admin downloads xlsx ({status})")

    # ---- employee deletion ----------------------------------------------
    worker_id = call("/api/me", worker_token)[1]["user"]["id"]
    admin_id = admin["user"]["id"]

    status, temp_user = call(
        "/api/users", admin_token, "POST",
        {"email": "temp@test.local", "name": "Temp", "password": "test1234",
         "role": "employee", "org_id": org_a},
    )
    check(status == 200, f"create a throwaway employee ({status})")

    status, _ = call(f"/api/users/{temp_user['id']}", worker_token, "DELETE")
    check(status == 403, f"employee cannot delete users ({status})")

    status, _ = call(f"/api/users/{temp_user['id']}", admin_token, "DELETE")
    check(status == 204, f"admin deletes an employee without entries ({status})")

    status, _ = call(f"/api/users/{temp_user['id']}", admin_token, "DELETE")
    check(status == 404, f"deleted employee is gone ({status})")

    status, _ = call(f"/api/users/{worker_id}", admin_token, "DELETE")
    check(status == 409, f"employee with entries is protected ({status})")

    status, _ = call(f"/api/users/{admin_id}", admin_token, "DELETE")
    check(status == 409, f"admin cannot delete themselves ({status})")

    status, off = call(f"/api/users/{worker_id}", admin_token, "PATCH", {"active": False})
    check(status == 200 and off["active"] is False, f"admin deactivates the employee ({status})")

    status, _ = call(
        "/api/auth/login", method="POST",
        body={"email": "worker@test.local", "password": "test1234"},
    )
    check(status == 401, f"deactivated employee cannot log in ({status})")

    status, team = call("/api/team", tokens["office@test.local"])
    check(
        any(r["user"]["email"] == "worker@test.local" and not r["user"]["active"] for r in team),
        "deactivated employee stays visible with the balance",
    )

    status, _ = call(f"/api/users/{worker_id}", admin_token, "PATCH", {"active": True})
    check(status == 200, f"employee reactivated ({status})")

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

    # sessions are wiped by an import, so log in again
    status, again = call(
        "/api/auth/login", method="POST",
        body={"email": "worker@test.local", "password": "test1234"},
    )
    check(status == 200, f"login after import ({status})")
    status, days = call("/api/days", again["token"])
    notes = [seg["note"] for entry in days for seg in entry["segments"] if seg["note"]]
    check(
        "Regal aufgefüllt (korrigiert)" in notes,
        "notes survive the JSON backup and restore",
    )

    print("\n".join(results))
    return 0 if all(line.startswith("PASS") for line in results) else 1


if __name__ == "__main__":
    sys.exit(main())
