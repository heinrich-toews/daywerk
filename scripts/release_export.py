#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# This file is part of Daywerk. See LICENSE for details.

"""Export the signed Android artifacts and update the release notes.

Copies the built APK (and AAB, when present) into the release folder, names them
``Daywerk-<version>-<date>.<ext>``, records size and SHA-256, and prepends a new
section to ``RELEASE-NOTES.md`` — newest release first.

The section body is your text (markdown, usually starting with ``###``); the
heading, server line and artifact table are generated.

    scripts/release_export.py --notes /tmp/notes-0.2.0.md
    scripts/release_export.py --date 2026-11-02 --server https://time.hgs-t.de

Build first:

    VITE_SERVER_URL=https://time.hgs-t.de pnpm android:build --apk --aab \
      --target aarch64 x86_64
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import shutil
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
TAURI_CONF = REPO / "apps/desktop/src-tauri/tauri.conf.json"
BUILD = REPO / "apps/desktop/src-tauri/gen/android/app/build/outputs"
APK = BUILD / "apk/universal/release/app-universal-release.apk"
AAB = BUILD / "bundle/universalRelease/app-universal-release.aab"
DIST = REPO / "apps/web-ui/dist"
MCP = REPO / "target/release/dw-mcp"

DEFAULT_OUT = pathlib.Path(
    "/home/twx/NextcloudTWx/03__Projekte/10__HGS/02__Buchungs_App/Releases"
)
MONTHS = [
    "Januar", "Februar", "März", "April", "Mai", "Juni",
    "Juli", "August", "September", "Oktober", "November", "Dezember",
]

HEADER = """# Daywerk – Release Notes

Neueste Änderungen stehen oben. Jede Version enthält die installierbaren
Artefakte im selben Ordner.

---
"""


def german_date(day: dt.date) -> str:
    return f"{day.day}. {MONTHS[day.month - 1]} {day.year}"


def human_size(path: pathlib.Path) -> str:
    size = path.stat().st_size / (1024 * 1024)
    return f"{size:.1f} MB".replace(".", ",")


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=pathlib.Path, default=DEFAULT_OUT, help="release folder")
    parser.add_argument("--date", type=dt.date.fromisoformat, default=dt.date.today(),
                        help="release date (YYYY-MM-DD), defaults to today")
    parser.add_argument("--server", default="https://time.hgs-t.de", help="server URL in the notes")
    parser.add_argument("--notes", type=pathlib.Path, help="markdown section body (optional)")
    args = parser.parse_args()

    if not APK.exists():
        print(f"error: {APK} not found — build the APK first", file=sys.stderr)
        return 1

    version = json.loads(TAURI_CONF.read_text())["version"]
    date = args.date
    heading = f"## {version} – {german_date(date)}"
    notes_path = args.out / "RELEASE-NOTES.md"
    if notes_path.exists() and heading in notes_path.read_text():
        print(f"error: {heading} is already in {notes_path}", file=sys.stderr)
        return 1

    body = args.notes.read_text().strip() if args.notes else "### Änderungen\n\n- …"
    first_line = body.splitlines()[0].strip() if body else ""
    if first_line.startswith("## "):
        print("error: --notes should contain the section body, not a heading", file=sys.stderr)
        return 1

    args.out.mkdir(parents=True, exist_ok=True)
    rows: list[str] = []
    for source, suffix in ((APK, "apk"), (AAB, "aab")):
        if not source.exists():
            continue
        name = f"Daywerk-{version}-{date.isoformat()}.{suffix}"
        target = args.out / name
        shutil.copy2(source, target)
        rows.append(f"| `{name}` | {human_size(target)} | `{sha256(target)}` |")

    # The MCP server runs on the workstation, not in the container.
    if MCP.exists():
        name = f"Daywerk-{version}-{date.isoformat()}-mcp-linux-x86_64"
        target = args.out / name
        shutil.copy2(MCP, target)
        target.chmod(0o755)
        rows.append(f"| `{name}` | {human_size(target)} | `{sha256(target)}` |")

    # The web build is the PWA; keep a copy so a release can be re-hosted.
    if DIST.is_dir():
        name = f"Daywerk-{version}-{date.isoformat()}-pwa"
        archive = shutil.make_archive(str(args.out / name), "zip", DIST)
        target = pathlib.Path(archive)
        rows.append(f"| `{target.name}` | {human_size(target)} | `{sha256(target)}` |")

    section = "\n".join(
        [
            heading,
            "",
            f"**Server:** {args.server} (Version {version})",
            "",
            "| Artefakt | Größe | SHA-256 |",
            "|---|---|---|",
            *rows,
            "",
            body,
            "",
        ]
    )

    if notes_path.exists():
        text = notes_path.read_text()
        marker = "\n---\n"
        index = text.find(marker)
        if index == -1:
            text = text.rstrip() + "\n\n" + section
        else:
            head = text[: index + len(marker)]
            rest = text[index + len(marker):].lstrip("\n")
            text = f"{head}\n{section}\n{rest}"
    else:
        text = HEADER + "\n" + section
    notes_path.write_text(text)

    print(f"exported {version} ({date.isoformat()}) to {args.out}")
    for row in rows:
        print(" ", row.strip("| "))
    print(f"notes: {notes_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
