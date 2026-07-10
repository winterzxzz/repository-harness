# US-082 Symphony Web UI Auto-Open

## Status

implemented

## Lane

normal

## Product Contract

Starting `harness-symphony web` must open the local Symphony controller in the
operator's default browser after the server binds successfully. Headless and
automated callers must be able to disable browser launch explicitly.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`
- `docs/SYMPHONY_QUICKSTART.md`
- `docs/superpowers/specs/2026-07-10-symphony-web-auto-open-design.md`
- `docs/stories/epics/E08-symphony-web-ui-controller/US-048-local-web-backend-api.md`
- `docs/stories/epics/E08-symphony-web-ui-controller/US-059-electron-desktop-shell.md`

## Acceptance Criteria

- `harness-symphony web` opens the system default browser exactly once after
  the loopback listener binds.
- The opened URL uses the listener's resolved address, including the assigned
  port when `--port 0` is used.
- `harness-symphony web --no-open` starts the server without invoking a browser.
- A browser-launch failure prints an actionable warning and does not terminate
  the running Web UI server.
- Bind failures remain fatal and never attempt browser launch.
- The Electron shell passes the headless option to its backend so it continues
  to create only its existing `BrowserWindow`.
- Existing board APIs, task execution, run state, and Web UI rendering remain
  unchanged.

## Design Notes

- Commands: `harness-symphony web`, `harness-symphony web --no-open`.
- Queries: no new query.
- API: no HTTP API changes.
- Tables: no database changes.
- Domain rules: server readiness precedes browser launch; browser launch is a
  convenience, not a server-health requirement.
- UI surfaces: existing browser controller root; no React changes expected.
- Runtime surfaces: Rust CLI/Web server boundary and Electron backend spawn
  arguments.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-082 --unit 1 --integration 1 --e2e 0 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | CLI option and browser-launch boundary tests cover default, opt-out, resolved URL, and non-fatal launch failure. |
| Integration | Existing `harness-symphony` Web UI server tests pass with auto-open disabled in test processes. |
| E2E | Existing browser E2E remains unchanged; no new React behavior is introduced. |
| Platform | Electron smoke proves its backend uses `--no-open`; a manual macOS smoke confirms browser mode opens the controller once. |
| Release | `cargo fmt --check`, targeted Rust tests, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, Electron smoke, and `git diff --check`. |

## Harness Delta

The Quick Start must document `--no-open` so agents and automated validation do
not accidentally launch a browser. No Harness policy or schema change is
required.

## Evidence

- Added default browser launch after the Web listener binds and resolves its
  actual address, including ephemeral `--port 0` assignments.
- Added `--no-open` for headless callers and made launch failures warn without
  terminating the bound server.
- Added Electron backend argument construction that defaults child Web servers
  to `--no-open`, preserving the existing single `BrowserWindow` behavior.
- `cargo test -p harness-symphony web_auto_open -- --nocapture` passed: 5 tests.
- `cargo test -p harness-symphony web -- --nocapture` passed: 47 tests.
- `cargo test --workspace` passed: 183 tests across workspace targets.
- `cargo fmt --check` and `cargo clippy --workspace -- -D warnings` passed.
- `npm --prefix crates/harness-symphony/web-ui run desktop:smoke` passed.
- Manual macOS smoke started browser mode on `http://127.0.0.1:50154` without a
  launch warning; `GET /health` returned `{"ok":true}` before shutdown.
- `scripts/bin/harness-cli story verify US-082` passed.
- `git diff --check` passed.
