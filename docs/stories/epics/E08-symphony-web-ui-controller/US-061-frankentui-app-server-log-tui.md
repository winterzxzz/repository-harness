# US-061 FrankenTUI App Server Log TUI

## Status

planned

## Lane

normal

## Product Contract

Technical maintainers can launch a terminal log viewer for local Symphony app
server activity, powered by FrankenTUI, without replacing the browser or
Electron review surfaces. The TUI must make live app-server logs easier to scan
while preserving raw log artifacts and existing Web UI log behavior.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`
- External reference: `https://github.com/Dicklesworthstone/frankentui`

## Acceptance Criteria

- A maintainer can start a local TUI log viewer from the Symphony CLI, using a
  command name and arguments that fit the existing `harness-symphony` command
  surface.
- The TUI displays live local app-server activity from the same durable sources
  used today, including `APP_SERVER_EVENTS.jsonl` for Codex App Server run
  events when a run is selected or active.
- The display supports readable scanning of timestamp/source/level/message or
  equivalent event metadata, plus a clear empty/error state when no log source
  exists.
- The TUI implementation follows FrankenTUI terminal constraints, including a
  single terminal writer, deterministic cleanup on exit, and bounded redraw
  work for high-volume logs.
- Existing browser and Electron log surfaces keep working and continue to expose
  raw artifacts for review/debugging.
- Dependency adoption is explicit: license, packaging, and update strategy for
  FrankenTUI are checked before adding it as a Rust dependency or vendored
  source.

## Design Notes

- Commands: likely add a subcommand such as `harness-symphony logs` or
  `harness-symphony tui logs`, with filters for run id and log source.
- Queries: reuse the existing run-state and run-artifact locations; do not add a
  second source of truth for app-server logs.
- API: no browser API change is required unless the implementation chooses to
  share a log formatting module across CLI and Web UI boundaries.
- Tables: none expected.
- Domain rules: operational logs are not Harness audit records. The TUI is a
  local inspection surface over existing files/state.
- UI surfaces: terminal TUI for maintainers, existing browser/Electron review
  log panel remains unchanged except for shared formatter reuse if useful.
- External dependency notes: FrankenTUI advertises inline terminal UI rendering,
  diff-based updates, RAII-style cleanup, and log-viewer examples; confirm the
  current crate/source shape during implementation.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-061 --unit 1 --integration 1 --e2e 1 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | Parser/formatter tests cover normal event lines, malformed JSONL, missing files, source filters, and high-volume truncation/windowing behavior. |
| Integration | CLI-level test or Rust integration test proves the TUI log command can read fixture run logs and exits cleanly without corrupting terminal state. |
| E2E | A smoke script or pseudo-terminal test starts the TUI against a fixture or live local run and verifies expected log text renders. |
| Platform | macOS terminal smoke proves cleanup and keyboard exit behavior; browser/Electron builds or tests prove existing log surfaces still pass. |
| Release | Not required. |

## Harness Delta

No process change. If dependency evaluation exposes recurring uncertainty around
third-party Rust TUI packages, record follow-up harness guidance for dependency
license and packaging checks.

## Evidence

Planned story only. Add implementation evidence after validation exists.
