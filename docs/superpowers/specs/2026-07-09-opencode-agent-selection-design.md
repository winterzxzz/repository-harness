# OpenCode Agent Selection — Design

Date: 2026-07-09
Status: Approved (Approach A)

## Problem

Symphony runs a story with exactly one agent adapter, resolved from the YAML
config at server start. The web UI hardcodes "Run with Codex". Users want to
run a task with OpenCode as well, choose the agent at run time, have the
choice remembered, and manage the default from a settings surface.

## Decision Summary

- Add a first-class `opencode` adapter alongside `custom` and `codex`.
- Allow a per-run agent override on the start-run API; the chosen agent
  becomes the new remembered default (global scope, not per-task).
- Persist the remembered default in the Symphony state DB as a `settings`
  key/value; the YAML `agent.adapter` remains the initial fallback.
- Web UI: split button `Run with <Agent> | ▾` on the board; a Settings
  surface with a Codex/OpenCode radio backed by `GET/PUT /api/settings`.

## Backend

### Adapter (`crates/harness-symphony/src/agent.rs`)

- Rename `codex_prompt` to `agent_prompt` (content unchanged; it is not
  Codex-specific).
- `run_opencode_agent`: spawn `opencode run --auto <prompt>` via
  `base_command` with the worktree as cwd — same simple wait-on-exit model as
  the custom adapter. Non-zero exit maps to `AgentError::CommandFailed` with
  trimmed stderr.
- `resolved_agent_command`: `opencode` defaults to
  `["opencode", "run", "--auto"]` when `agent.command` is empty.
- `agent_adapter_status` and doctor gain an `opencode` case (binary presence
  check).
- The unsupported-adapter error message lists `custom, codex, opencode`.

### Settings + per-run agent (`state.rs`, `web.rs`, `run.rs`)

- State DB gains a `settings` table (`key TEXT PRIMARY KEY, value TEXT`).
  Key `default_agent` holds `codex` or `opencode`. Missing key → fall back to
  the resolved config `agent.adapter`.
- `GET /api/settings` → `{ "default_agent": "<value>" }` (resolved with
  fallback). `PUT /api/settings` accepts the same shape, validates the value
  against known adapters, persists.
- Start-run request body accepts optional `agent`. When present: validate,
  use for this run, and persist as `default_agent` (choosing at run time IS
  remembering). When absent: use `default_agent`.
- The run record stores the agent used (`agent` column/field) so the UI and
  run log can name the right agent.

## Web UI (`crates/harness-symphony/web-ui`)

- `board.tsx`: split button — primary segment `▶ Run with <Agent>` starts
  immediately with the current default; the chevron opens a dropdown listing
  Codex and OpenCode. Picking one starts the run with it and updates the
  default. Confirm dialog text names the chosen agent.
- Settings surface: sidebar entry opening a simple panel with a radio group
  (Codex / OpenCode) wired to `GET/PUT /api/settings`, with toast feedback.
- `run-log.ts`: replace hardcoded "Codex" strings with the run's agent name.
- `api.ts`/`types.ts`: settings endpoints, `agent` on start-run and run
  records.

## Error handling

- Unknown `agent` value on either endpoint → 400 with the allowed list.
- `opencode` binary missing → run fails with the existing CommandFailed
  surface; doctor reports absence ahead of time.
- Settings read/write failures surface as 500 via existing WebError paths.

## Testing

- Rust unit: opencode dispatch with a fake binary (mirroring existing fake
  codex app-server tests); settings get/put round-trip and fallback;
  start-run with agent override persists the default; invalid agent → 400.
- Web-ui E2E: dropdown pick sends `agent: "opencode"`; settings change flips
  the Run button label.

## Intake

Lane: normal with stronger validation (2 flags: public contracts, external
systems). Story packet created under `docs/stories/`.

## Out of scope (YAGNI)

- Per-task remembered agent.
- OpenCode server/attach mode (`opencode serve`); headless CLI only.
- Model/provider selection within OpenCode.
