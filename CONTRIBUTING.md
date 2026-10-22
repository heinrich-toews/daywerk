# Contributing

Thanks for helping with Daywerk. This project is small and opinionated; the
notes below keep it that way.

## Developer Certificate of Origin (DCO)

Every commit **must** carry a `Signed-off-by` trailer. It is the statement that
you wrote the change (or have the right to submit it) under the project licence,
[AGPL-3.0-only](LICENSE).

Add it with `-s`:

```sh
git commit -s -m "feat: describe the change"
```

which produces:

```
feat: describe the change

Signed-off-by: Your Name <you@example.org>
```

The identity comes from `git config user.name` / `user.email`, so set them
correctly. Commits without the trailer are rejected by the repository hook:

```sh
scripts/install-git-hooks.sh
```

Run that once per clone. It installs a `commit-msg` hook that refuses any commit
without a sign-off (merge commits are exempt).

## Working on the code

```sh
pnpm install
pnpm test              # Rust workspace + web UI unit tests
pnpm build             # web UI bundle
cargo run -p dw-server # API on :9095

python3 scripts/api-smoke.py   # end-to-end checks, needs a fresh database
```

Requirements: Rust ≥ 1.85, Node ≥ 22, pnpm. Android work additionally needs the
toolchain described in [docs/android.md](docs/android.md).

## Before you push

1. `cargo fmt --all` and `pnpm build` are clean.
2. `pnpm test` passes.
3. `scripts/api-smoke.py` passes against a fresh database if you touched the API.
4. The commit is signed off and its message says **why**, not just what.

## Project layout

```
crates/dw-core      pure domain logic (punch state machine, contract maths)
crates/dw-storage   SQLite persistence and migrations
crates/dw-server    axum REST API, auth, roles, reports
crates/dw-export    XLSX and PDF rendering
apps/web-ui         SolidJS + Tailwind frontend
apps/desktop        Tauri shell (Android/iOS/desktop)
docs/               Android setup, decisions
```

Design decisions worth reading before changing behaviour live in
[docs/decisions.md](docs/decisions.md).

## Commit messages

`type: short imperative summary`, then a body explaining the reasoning and any
trade-off. Types used so far: `feat`, `fix`, `refactor`, `chore`, `docs`, `test`.
