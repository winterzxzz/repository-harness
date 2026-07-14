# US-078 OpenCode Agent Selection

## Status

implemented

## Lane

normal

## Product Contract

A user can run a Ready story with either Codex or OpenCode. The board Run
button is a split button: the primary segment runs with the current default
agent; the dropdown lists Codex and OpenCode, and picking one runs with it
and remembers it as the new default. A Settings surface lets the user set the
default agent directly. The remembered default is global (not per-task) and
survives server restarts.

## Relevant Product Docs

- `docs/superpowers/specs/2026-07-09-opencode-agent-selection-design.md`

## Acceptance Criteria

- `agent.adapter: opencode` runs `opencode run --auto <prompt>` in the run
  worktree and maps non-zero exit to a failed run with stderr detail.
- Start-run API accepts optional `agent` (`codex` | `opencode`); the value is
  validated, used for that run, and persisted as `default_agent` in the state
  DB. Unknown values return 400 with the allowed list.
- `GET /api/settings` returns the resolved default agent (state DB value,
  falling back to config `agent.adapter`); `PUT /api/settings` updates it.
- Run records store the agent used; the web UI run log names that agent
  instead of hardcoding "Codex".
- OpenCode retains `AGENT_OUTPUT.log` and also emits sequenced normalized
  `RUN_EVENTS.jsonl` output for live cursor polling in task detail.
- Board split button and Settings radio reflect and update the default.

## Design Notes

- Commands: `opencode run --auto <prompt>` (headless, cwd = worktree).
- Queries: settings get/put; run record includes `agent`.
- API: `POST` start-run gains optional `agent`; new `GET/PUT /api/settings`.
- Tables: `settings(key TEXT PRIMARY KEY, value TEXT)` in `.symphony/state.db`.
- Domain rules: choosing an agent at run time is remembering it.
- UI surfaces: board split button, sidebar Settings panel, run log labels, and
  normalized live output in active task detail.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-078 --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | Rust tests: opencode dispatch via fake binary; settings round-trip and fallback; start-run agent override persists default; invalid agent → 400 |
| Integration | Web endpoint tests exercising settings and start-run with agent override against a temp state DB |
| E2E | web-ui tests: dropdown pick sends `agent: "opencode"`; settings change flips Run button label |
| Platform | n/a |
| Release | n/a |

## Harness Delta

None yet.

## Evidence

- `cargo test -p harness-symphony`: 112 passed (opencode adapter fake-binary
  tests, settings round-trip, start-run agent override persisting the default,
  invalid agent 400).
- `crates/harness-symphony/web-ui`: `npx tsc --noEmit` clean;
  `npx playwright test`: 27 passed (agent dropdown runs with opencode and
  remembers the choice; settings view saves the default agent and relabels
  the run button).
